use crate::error::PluginError;
use crate::json_struct;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// Architecture of the host environment.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HostArch {
    X86,
    #[default]
    X64,
    Arm,
    Arm64,
    Mips,
    Mips64,
    Powerpc,
    Powerpc64,
    S390x,
}

impl HostArch {
    pub fn to_rust_arch(&self) -> String {
        match self {
            Self::X64 => "x86_64".into(),
            Self::Arm64 => "aarch64".into(),
            _ => self.to_string(),
        }
    }
}

impl fmt::Display for HostArch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", format!("{:?}", self).to_lowercase())
    }
}

impl FromStr for HostArch {
    type Err = PluginError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "x86" => Ok(Self::X86),
            "x86_64" => Ok(Self::X64),
            "arm" => Ok(Self::Arm),
            "aarch64" => Ok(Self::Arm64),
            "mips" => Ok(Self::Mips),
            "mips64" => Ok(Self::Mips64),
            "powerpc" => Ok(Self::Powerpc),
            "powerpc64" => Ok(Self::Powerpc64),
            "s390x" => Ok(Self::S390x),
            arch => Err(PluginError::Message(format!(
                "Unsupported architecture {arch}."
            ))),
        }
    }
}

/// Operating system of the host environment.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HostOS {
    #[default]
    Linux,
    MacOS,
    FreeBSD,
    NetBSD,
    OpenBSD,
    Windows,
}

impl HostOS {
    pub fn is_bsd(&self) -> bool {
        matches!(self, Self::FreeBSD | Self::NetBSD | Self::OpenBSD)
    }

    pub fn is_linux(&self) -> bool {
        !matches!(self, Self::MacOS | Self::Windows)
    }

    pub fn to_rust_os(&self) -> String {
        self.to_string()
    }
}

impl fmt::Display for HostOS {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", format!("{:?}", self).to_lowercase())
    }
}

impl FromStr for HostOS {
    type Err = PluginError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "linux" => Ok(Self::Linux),
            "macos" => Ok(Self::MacOS),
            "freebsd" => Ok(Self::FreeBSD),
            "netbsd" => Ok(Self::NetBSD),
            "openbsd" => Ok(Self::OpenBSD),
            "windows" => Ok(Self::Windows),
            os => Err(PluginError::Message(format!(
                "Unsupported operating system {os}."
            ))),
        }
    }
}

/// Container for WASI virtual paths that also keep a reference to the original real path.
#[derive(Clone, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(into = "String", from = "String")]
pub struct VirtualPath {
    virt: PathBuf,
    real: Option<PathBuf>,
}

impl VirtualPath {
    pub fn new(virt: impl Into<PathBuf>, real: impl Into<PathBuf>) -> Self {
        Self {
            virt: virt.into(),
            real: Some(real.into()),
        }
    }

    pub fn compat(virt: impl Into<PathBuf>) -> Self {
        Self {
            virt: virt.into(),
            real: None,
        }
    }

    pub fn real_path(&self) -> &Path {
        self.real.as_ref().expect("No real path.")
    }

    pub fn virtual_path(&self) -> &Path {
        &self.virt
    }
}

impl Deref for VirtualPath {
    type Target = PathBuf;

    fn deref(&self) -> &Self::Target {
        &self.virt
    }
}

impl DerefMut for VirtualPath {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.virt
    }
}

impl From<String> for VirtualPath {
    fn from(path: String) -> Self {
        let mut parts = path.splitn(2, "::");

        Self {
            virt: parts.next().unwrap_or_default().into(),
            real: parts.next().map(|p| p.into()),
        }
    }
}

impl From<VirtualPath> for String {
    fn from(path: VirtualPath) -> Self {
        if let Some(real) = &path.real {
            format!("{}::{}", path.virt.display(), real.display())
        } else {
            format!("{}", path.virt.display())
        }
    }
}

impl fmt::Display for VirtualPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.virt.display())
    }
}

impl fmt::Debug for VirtualPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

json_struct!(
    pub struct UserConfigSettings {
        pub auto_clean: bool,
        pub auto_install: bool,
        pub node_intercept_globals: bool,
    }
);
