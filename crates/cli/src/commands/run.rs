use crate::commands::install::{internal_install, InstallArgs};
use crate::error::ProtoCliError;
use crate::helpers::ProtoResource;
use clap::Args;
use miette::IntoDiagnostic;
use proto_core::{detect_version, Id, ProtoError, Tool, UnresolvedVersionSpec};
use proto_pdk_api::{ExecutableConfig, RunHook};
use proto_shim::exec_command_and_replace;
use starbase::system;
use std::env;
use std::ffi::OsStr;
use std::process::Command;
use system_env::create_process_command;
use tracing::debug;

#[derive(Args, Clone, Debug)]
pub struct RunArgs {
    #[arg(required = true, help = "ID of tool")]
    id: Id,

    #[arg(help = "Version or alias of tool")]
    spec: Option<UnresolvedVersionSpec>,

    #[arg(long, help = "Name of an alternate (secondary) binary to run")]
    alt: Option<String>,

    // Passthrough args (after --)
    #[arg(
        last = true,
        help = "Arguments to pass through to the underlying command"
    )]
    passthrough: Vec<String>,
}

fn is_trying_to_self_upgrade(tool: &Tool, args: &[String]) -> bool {
    if tool.metadata.self_upgrade_commands.is_empty() {
        return false;
    }

    for arg in args {
        // Find first non-option arg
        if arg.starts_with('-') {
            continue;
        }

        // And then check if an upgrade command
        return tool.metadata.self_upgrade_commands.contains(arg);
    }

    false
}

fn get_executable(tool: &Tool, args: &RunArgs) -> miette::Result<ExecutableConfig> {
    let tool_dir = tool.get_tool_dir();

    // Run an alternate executable (via shim)
    if let Some(alt_name) = &args.alt {
        for location in tool.get_shim_locations()? {
            if location.name == *alt_name {
                let Some(exe_path) = &location.config.exe_path else {
                    continue;
                };

                let alt_path = tool_dir.join(exe_path);

                if alt_path.exists() {
                    debug!(
                        bin = alt_name,
                        path = ?alt_path,
                        "Received an alternate binary to run with",
                    );

                    return Ok(ExecutableConfig {
                        exe_path: Some(alt_path),
                        ..location.config
                    });
                }
            }
        }

        return Err(ProtoCliError::MissingRunAltBin {
            bin: alt_name.to_owned(),
            path: tool_dir,
        }
        .into());
    }

    // Otherwise use the primary
    let mut config = tool
        .get_exe_location()?
        .expect("Required executable information missing!")
        .config;

    config.exe_path = Some(tool_dir.join(config.exe_path.as_ref().unwrap()));

    Ok(config)
}

fn create_command<I: IntoIterator<Item = A>, A: AsRef<OsStr>>(
    tool: &Tool,
    exe_config: &ExecutableConfig,
    args: I,
) -> miette::Result<Command> {
    let exe_path = exe_config.exe_path.as_ref().unwrap();
    let args = args
        .into_iter()
        .map(|arg| arg.as_ref().to_os_string())
        .collect::<Vec<_>>();

    let command = if let Some(parent_exe_path) = &exe_config.parent_exe_name {
        let mut exe_args = vec![exe_path.as_os_str().to_os_string()];
        exe_args.extend(args);

        debug!(
            bin = ?parent_exe_path,
            args = ?exe_args,
            pid = std::process::id(),
            "Running {}", tool.get_name(),
        );

        create_process_command(parent_exe_path, exe_args)
    } else {
        debug!(
            bin = ?exe_path,
            args = ?args,
            pid = std::process::id(),
            "Running {}", tool.get_name(),
        );

        create_process_command(exe_path, args)
    };

    Ok(command)
}

#[system]
pub async fn run(args: ArgsRef<RunArgs>, proto: ResourceRef<ProtoResource>) -> SystemResult {
    let mut tool = proto.load_tool(&args.id).await?;

    // Avoid running the tool's native self-upgrade as it conflicts with proto
    if is_trying_to_self_upgrade(&tool, &args.passthrough) {
        return Err(ProtoCliError::NoSelfUpgrade {
            command: format!("proto install {} --pin", tool.id),
            tool: tool.get_name().to_owned(),
        }
        .into());
    }

    let version = detect_version(&tool, args.spec.clone()).await?;

    // Check if installed or install
    if !tool.is_setup(&version).await? {
        let config = tool.proto.load_config()?;

        if !config.settings.auto_install {
            return Err(ProtoError::MissingToolForRun {
                tool: tool.get_name().to_owned(),
                version: version.to_string(),
                command: format!("proto install {} {}", tool.id, tool.get_resolved_version()),
            }
            .into());
        }

        // Install the tool
        debug!("Auto-install setting is configured, attempting to install");

        tool = internal_install(
            proto,
            InstallArgs {
                canary: false,
                id: args.id.clone(),
                pin: false,
                passthrough: vec![],
                spec: Some(tool.get_resolved_version().to_unresolved_spec()),
            },
            Some(tool),
        )
        .await?;
    }

    // Determine the binary path to execute
    let exe_config = get_executable(&tool, args)?;
    let exe_path = exe_config.exe_path.as_ref().unwrap();

    // Run before hook
    tool.run_hook("pre_run", || RunHook {
        context: tool.create_context(),
        passthrough_args: args.passthrough.clone(),
    })?;

    // Create and run the command
    let mut command = create_command(&tool, &exe_config, &args.passthrough)?;

    command
        .env(
            format!("{}_VERSION", tool.get_env_var_prefix()),
            tool.get_resolved_version().to_string(),
        )
        .env(
            format!("{}_BIN", tool.get_env_var_prefix()),
            exe_path.to_string_lossy().to_string(),
        );

    // Update the last used timestamp
    if env::var("PROTO_SKIP_USED_AT").is_err() {
        tool.manifest.track_used_at(tool.get_resolved_version());

        // Ignore failures to not disrupt the user
        let _ = tool.manifest.save();
    }

    // Must be the last line!
    exec_command_and_replace(command).into_diagnostic()?;
}
