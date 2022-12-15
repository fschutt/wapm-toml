//! The Manifest file is where the core metadata of a wapm package lives

use semver::Version;
use serde::{de::Error as _, Deserialize, Serialize};
use std::collections::{hash_map::HashMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub mod rust;

/// The ABI is a hint to WebAssembly runtimes about what additional imports to insert.
/// It currently is only used for validation (in the validation subcommand).  The default value is `None`.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Abi {
    #[serde(rename = "emscripten")]
    Emscripten,
    #[serde(rename = "none")]
    None,
    #[serde(rename = "wasi")]
    Wasi,
    #[serde(rename = "wasm4")]
    WASM4,
}

impl Abi {
    pub fn to_str(&self) -> &str {
        match self {
            Abi::Emscripten => "emscripten",
            Abi::Wasi => "wasi",
            Abi::WASM4 => "wasm4",
            Abi::None => "generic",
        }
    }
    pub fn is_none(&self) -> bool {
        self == &Abi::None
    }
    pub fn from_name(name: &str) -> Self {
        match name.to_lowercase().as_ref() {
            "emscripten" => Abi::Emscripten,
            "wasi" => Abi::Wasi,
            _ => Abi::None,
        }
    }
}

impl fmt::Display for Abi {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_str())
    }
}

impl Default for Abi {
    fn default() -> Self {
        Abi::None
    }
}

/// The name of the manifest file. This is hard-coded for now.
pub static MANIFEST_FILE_NAME: &str = "wapm.toml";
pub static PACKAGES_DIR_NAME: &str = "wapm_packages";

pub static README_PATHS: &[&str; 5] = &[
    "README",
    "README.md",
    "README.markdown",
    "README.mdown",
    "README.mkdn",
];

pub static LICENSE_PATHS: &[&str; 3] = &["LICENSE", "LICENSE.md", "COPYING"];

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageName {
    pub namespace: String,
    pub name: String,
}

impl PackageName {
    /// Parses the package name from a `namespace/name` format
    pub fn parse(s: &str) -> Result<Self, &'static str> {
        if !s.contains('/') {
            return Err("no / in package name");
        }
        let mut split = s.split('/');
        let name = split.next().ok_or("no name in package name")?.to_string();
        let namespace = split
            .next()
            .ok_or("no namespace in package name")?
            .to_string();
        Ok(Self { name, namespace })
    }

    /// Returns the `namespace/name` package name
    pub fn to_string(&self) -> String {
        format!("{}", self)
    }
}

impl std::str::FromStr for PackageName {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl fmt::Display for PackageName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}", self.namespace, self.name)
    }
}

mod serde_package_name {
    use super::PackageName;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(package: &PackageName, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.serialize_str(&package.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<PackageName, D::Error>
    where
        D: Deserializer<'de>,
    {
        let buf = String::deserialize(deserializer)?;
        PackageName::parse(&buf).map_err(serde::de::Error::custom)
    }
}

/// Describes a command for a wapm module
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Package {
    #[serde(with = "serde_package_name")]
    pub name: PackageName,
    pub version: Version,
    pub description: String,
    pub license: Option<String>,
    /// The location of the license file, useful for non-standard licenses
    #[serde(rename = "license-file")]
    pub license_file: Option<PathBuf>,
    pub readme: Option<PathBuf>,
    pub repository: Option<String>,
    pub homepage: Option<String>,
    #[serde(rename = "wasmer-extra-flags")]
    pub wasmer_extra_flags: Option<String>,
    #[serde(
        rename = "disable-command-rename",
        default,
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub disable_command_rename: bool,
    /// Unlike, `disable-command-rename` which prevents `wapm run <Module name>`,
    /// this flag enables the command rename of `wapm run <COMMAND_NAME>` into
    /// just `<COMMAND_NAME>`. This is useful for programs that need to inspect
    /// their `argv[0]` names and when the command name matches their executable
    /// name.
    #[serde(
        rename = "rename-commands-to-raw-command-name",
        default,
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub rename_commands_to_raw_command_name: bool,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Command {
    V1(CommandV1),
    V2(CommandV2),
}

impl Command {
    pub fn get_name(&self) -> String {
        match self {
            Self::V1(c) => c.name.clone(),
            Self::V2(c) => c.name.clone(),
        }
    }

    pub fn get_module(&self) -> String {
        match self {
            Self::V1(c) => c.module.clone(),
            // TODO(felix): how to migrate to the new API?
            Self::V2(_) => String::new(),
        }
    }

    pub fn get_package(&self) -> Option<String> {
        match self {
            Self::V1(c) => c.package.clone(),
            // TODO(felix): how to migrate to the new version / "kind" API?
            Self::V2(_) => None,
        }
    }

    pub fn get_main_args(&self) -> Option<String> {
        match self {
            Self::V1(c) => c.main_args.clone(),
            // TODO(felix): how to migrate to the new API?
            // Self::V2(c) => serde_json::to_string(&c.annotations)
            Self::V2(_) => None,
        }
    }
}

/// Describes a command for a wapm module
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)] // Note: needed to prevent accidentally parsing
                              // a CommandV2 as a CommandV1
pub struct CommandV1 {
    pub name: String,
    pub module: String,
    pub main_args: Option<String>,
    pub package: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct CommandV2 {
    pub name: String,
    pub module: String,
    pub runner: String,
    pub annotations: Option<CommandAnnotations>,
}

impl CommandV2 {
    pub fn get_annotations(&self, basepath: &Path) -> Result<Option<serde_cbor::Value>, String> {
        match self.annotations.as_ref() {
            Some(CommandAnnotations::Raw(v)) => Ok(Some(toml_to_cbor_value(v))),
            Some(CommandAnnotations::File(FileCommandAnnotations { file, kind })) => {
                let path = basepath.join(file.clone());
                let file = std::fs::read_to_string(&path).map_err(|e| {
                    format!(
                        "Error reading {:?}.annotation ({:?}): {e}",
                        self.name,
                        path.display()
                    )
                })?;
                match kind {
                    FileKind::Json => {
                        let value: serde_json::Value =
                            serde_json::from_str(&file).map_err(|e| {
                                format!(
                                    "Error reading {:?}.annotation ({:?}): {e}",
                                    self.name,
                                    path.display()
                                )
                            })?;
                        Ok(Some(json_to_cbor_value(&value)))
                    }
                    FileKind::Yaml => {
                        let value: serde_yaml::Value =
                            serde_yaml::from_str(&file).map_err(|e| {
                                format!(
                                    "Error reading {:?}.annotation ({:?}): {e}",
                                    self.name,
                                    path.display()
                                )
                            })?;
                        Ok(Some(yaml_to_cbor_value(&value)))
                    }
                }
            }
            None => Ok(None),
        }
    }
}

pub fn toml_to_cbor_value(val: &toml::Value) -> serde_cbor::Value {
    match val {
        toml::Value::String(s) => serde_cbor::Value::Text(s.clone()),
        toml::Value::Integer(i) => serde_cbor::Value::Integer(*i as i128),
        toml::Value::Float(f) => serde_cbor::Value::Float(*f),
        toml::Value::Boolean(b) => serde_cbor::Value::Bool(*b),
        toml::Value::Datetime(d) => serde_cbor::Value::Text(format!("{}", d)),
        toml::Value::Array(sq) => {
            serde_cbor::Value::Array(sq.iter().map(toml_to_cbor_value).collect())
        }
        toml::Value::Table(m) => serde_cbor::Value::Map(
            m.iter()
                .map(|(k, v)| (serde_cbor::Value::Text(k.clone()), toml_to_cbor_value(v)))
                .collect(),
        ),
    }
}

pub fn json_to_cbor_value(val: &serde_json::Value) -> serde_cbor::Value {
    match val {
        serde_json::Value::Null => serde_cbor::Value::Null,
        serde_json::Value::Bool(b) => serde_cbor::Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                serde_cbor::Value::Integer(i as i128)
            } else if let Some(u) = n.as_u64() {
                serde_cbor::Value::Integer(u as i128)
            } else if let Some(f) = n.as_f64() {
                serde_cbor::Value::Float(f)
            } else {
                serde_cbor::Value::Null
            }
        }
        serde_json::Value::String(s) => serde_cbor::Value::Text(s.clone()),
        serde_json::Value::Array(sq) => {
            serde_cbor::Value::Array(sq.iter().map(json_to_cbor_value).collect())
        }
        serde_json::Value::Object(m) => serde_cbor::Value::Map(
            m.iter()
                .map(|(k, v)| (serde_cbor::Value::Text(k.clone()), json_to_cbor_value(v)))
                .collect(),
        ),
    }
}

pub fn yaml_to_cbor_value(val: &serde_yaml::Value) -> serde_cbor::Value {
    match val {
        serde_yaml::Value::Null => serde_cbor::Value::Null,
        serde_yaml::Value::Bool(b) => serde_cbor::Value::Bool(*b),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                serde_cbor::Value::Integer(i as i128)
            } else if let Some(u) = n.as_u64() {
                serde_cbor::Value::Integer(u as i128)
            } else if let Some(f) = n.as_f64() {
                serde_cbor::Value::Float(f)
            } else {
                serde_cbor::Value::Null
            }
        }
        serde_yaml::Value::String(s) => serde_cbor::Value::Text(s.clone()),
        serde_yaml::Value::Sequence(sq) => {
            serde_cbor::Value::Array(sq.iter().map(yaml_to_cbor_value).collect())
        }
        serde_yaml::Value::Mapping(m) => serde_cbor::Value::Map(
            m.iter()
                .map(|(k, v)| (yaml_to_cbor_value(k), yaml_to_cbor_value(v)))
                .collect(),
        ),
        serde_yaml::Value::Tagged(tag) => yaml_to_cbor_value(&tag.value),
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
#[repr(C)]
pub enum CommandAnnotations {
    File(FileCommandAnnotations),
    Raw(toml::Value),
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct FileCommandAnnotations {
    pub file: PathBuf,
    pub kind: FileKind,
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Ord, Eq, Deserialize, Serialize)]
pub enum FileKind {
    #[serde(rename = "yaml")]
    Yaml,
    #[serde(rename = "json")]
    Json,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Module {
    pub name: String,
    pub source: PathBuf,
    #[serde(default = "Abi::default", skip_serializing_if = "Abi::is_none")]
    pub abi: Abi,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interfaces: Option<HashMap<String, String>>,
    pub bindings: Option<Bindings>,
}

/// The interface exposed by a [`Module`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Bindings {
    Wit(WitBindings),
    Wai(WaiBindings),
}

impl Bindings {
    /// Get all files that make up this interface.
    ///
    /// For all binding types except [`WitBindings`], this will recursively
    /// look for any files that are imported.
    ///
    /// The caller can assume that any path that was referenced exists.
    pub fn referenced_files(&self, base_directory: &Path) -> Result<Vec<PathBuf>, ImportsError> {
        match self {
            Bindings::Wit(WitBindings { wit_exports, .. }) => {
                // Note: we explicitly don't support imported files with WIT
                // because wit-bindgen's wit-parser crate isn't on crates.io.

                let path = base_directory.join(wit_exports);

                if path.exists() {
                    Ok(vec![path])
                } else {
                    Err(ImportsError::FileNotFound(path))
                }
            }
            Bindings::Wai(wai) => wai.referenced_files(base_directory),
        }
    }
}

impl Serialize for Bindings {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Bindings::Wit(w) => w.serialize(serializer),
            Bindings::Wai(w) => w.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for Bindings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = toml::Value::deserialize(deserializer)?;

        let keys = ["wit-bindgen", "wai-version"];
        let [wit_bindgen, wai_version] = keys.map(|key| value.get(key).is_some());

        match (wit_bindgen, wai_version) {
            (true, false) => WitBindings::deserialize(value)
                .map(Bindings::Wit)
                .map_err(D::Error::custom),
            (false, true) => WaiBindings::deserialize(value)
                .map(Bindings::Wai)
                .map_err(D::Error::custom),
            (true, true) | (false, false) => {
                let msg = format!(
                    "expected one of \"{}\" to be provided, but not both",
                    keys.join("\" or \""),
                );
                Err(D::Error::custom(msg))
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct WitBindings {
    /// The version of the WIT format being used.
    pub wit_bindgen: Version,
    /// The `*.wit` file's location on disk.
    pub wit_exports: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct WaiBindings {
    /// The version of the WAI format being used.
    pub wai_version: Version,
    /// The `*.wai` file defining the interface this package exposes.
    pub exports: Option<PathBuf>,
    /// The `*.wai` files for any functionality this package imports from the
    /// host.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub imports: Vec<PathBuf>,
}

impl WaiBindings {
    fn referenced_files(&self, base_directory: &Path) -> Result<Vec<PathBuf>, ImportsError> {
        let WaiBindings {
            exports, imports, ..
        } = self;

        // Note: WAI files may import other WAI files, so we start with all
        // WAI files mentioned in the wapm.toml then recursively add their
        // imports.

        let initial_paths = exports
            .iter()
            .chain(imports)
            .map(|relative_path| base_directory.join(relative_path));

        let mut to_check: Vec<PathBuf> = Vec::new();

        for path in initial_paths {
            if !path.exists() {
                return Err(ImportsError::FileNotFound(path));
            }
            to_check.push(path);
        }

        let mut files = BTreeSet::new();

        while let Some(path) = to_check.pop() {
            if files.contains(&path) {
                continue;
            }

            to_check.extend(get_imported_wai_files(&path)?);
            files.insert(path);
        }

        Ok(files.into_iter().collect())
    }
}

/// Parse a `*.wai` file to find the absolute path for any other `*.wai` files
/// it may import, relative to the original `*.wai` file.
///
/// This function makes sure any imported files exist.
fn get_imported_wai_files(path: &Path) -> Result<Vec<PathBuf>, ImportsError> {
    let _wai_src = std::fs::read_to_string(path).map_err(|error| ImportsError::Read {
        path: path.to_path_buf(),
        error,
    })?;

    let parent_dir = path.parent()
            .expect("All paths should have a parent directory because we joined them relative to the base directory");

    // TODO(Michael-F-Bryan): update the wai-parser crate to give you access to
    // the imported interfaces. For now, we just pretend there are no import
    // statements in the *.wai file.
    let raw_imports: Vec<String> = Vec::new();

    // Note: imported paths in a *.wai file are all relative, so we need to
    // resolve their absolute path relative to the original *.wai file.
    let mut resolved_paths = Vec::new();

    for imported in raw_imports {
        let absolute_path = parent_dir.join(imported);

        if !absolute_path.exists() {
            return Err(ImportsError::ImportedFileNotFound {
                path: absolute_path,
                referenced_by: path.to_path_buf(),
            });
        }

        resolved_paths.push(absolute_path);
    }

    Ok(resolved_paths)
}

#[derive(Debug, thiserror::Error)]
pub enum ImportsError {
    #[error(
        "The \"{}\" mentioned in the manifest doesn't exist",
        _0.display(),
    )]
    FileNotFound(PathBuf),
    #[error(
        "The \"{}\" imported by \"{}\" doesn't exist",
        path.display(),
        referenced_by.display(),
    )]
    ImportedFileNotFound {
        path: PathBuf,
        referenced_by: PathBuf,
    },
    #[error("Unable to parse \"{}\" as a WAI file", path.display())]
    WaiParse { path: PathBuf },
    #[error("Unable to read \"{}\"", path.display())]
    Read {
        path: PathBuf,
        #[source]
        error: std::io::Error,
    },
}

/// The manifest represents the file used to describe a Wasm package.
///
/// The `module` field represents the wasm file to be published.
///
/// The `source` is used to create bundles with the `fs` section.
///
/// The `fs` section represents fs assets that will be made available to the
/// program relative to its starting current directory (there may be issues with WASI).
/// These are pairs of paths.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Manifest {
    pub package: Package,
    pub dependencies: Option<HashMap<String, String>>,
    pub module: Option<Vec<Module>>,
    pub command: Option<Vec<Command>>,
    /// Of the form Guest -> Host path
    pub fs: Option<HashMap<String, PathBuf>>,
    /// private data
    /// store the directory path of the manifest file for use later accessing relative path fields
    #[serde(skip)]
    pub base_directory_path: PathBuf,
}

impl Manifest {
    pub fn parse(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }

    fn locate_file(path: &Path, candidates: &[&str]) -> Option<PathBuf> {
        for filename in candidates {
            let path_buf = path.join(filename);
            if path_buf.exists() {
                return Some(filename.into());
            }
        }
        None
    }

    /// Construct a manifest by searching in the specified directory for a manifest file
    pub fn find_in_directory<T: AsRef<Path>>(path: T) -> Result<Self, ManifestError> {
        if !path.as_ref().is_dir() {
            return Err(ManifestError::MissingManifest(
                path.as_ref().to_string_lossy().to_string(),
            ));
        }
        let manifest_path_buf = path.as_ref().join(MANIFEST_FILE_NAME);
        let contents = std::fs::read_to_string(&manifest_path_buf).map_err(|_e| {
            ManifestError::MissingManifest(manifest_path_buf.to_string_lossy().to_string())
        })?;
        let mut manifest: Self = toml::from_str(contents.as_str())
            .map_err(|e| ManifestError::TomlParseError(e.to_string()))?;
        if manifest.package.readme.is_none() {
            manifest.package.readme = Self::locate_file(path.as_ref(), &README_PATHS[..]);
        }
        if manifest.package.license_file.is_none() {
            manifest.package.license_file = Self::locate_file(path.as_ref(), &LICENSE_PATHS[..]);
        }
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<(), ManifestError> {
        let module_map = self
            .module
            .as_ref()
            .map(|modules| {
                modules
                    .iter()
                    .map(|module| (module.name.clone(), module.clone()))
                    .collect::<HashMap<String, Module>>()
            })
            .unwrap_or_default();

        if let Some(ref commands) = self.command {
            for command in commands {
                if let Some(module) = module_map.get(&command.get_module()) {
                    if module.abi == Abi::None && module.interfaces.is_none() {
                        return Err(ManifestError::ValidationError(ValidationError::MissingABI(
                            command.get_name(),
                            module.name.clone(),
                        )));
                    }
                } else {
                    return Err(ManifestError::ValidationError(
                        ValidationError::MissingModuleForCommand(
                            command.get_name(),
                            command.get_module(),
                        ),
                    ));
                }
            }
        }
        Ok(())
    }

    /// add a dependency
    pub fn add_dependency(&mut self, dependency_name: String, dependency_version: String) {
        let dependencies = self.dependencies.get_or_insert(Default::default());
        dependencies.insert(dependency_name, dependency_version);
    }

    /// remove dependency by package name
    pub fn remove_dependency(&mut self, dependency_name: &str) -> Option<String> {
        let dependencies = self.dependencies.get_or_insert(Default::default());
        dependencies.remove(dependency_name)
    }

    pub fn to_string(&self) -> anyhow::Result<String> {
        Ok(toml::to_string(self)?)
    }

    pub fn manifest_path(&self) -> PathBuf {
        self.base_directory_path.join(MANIFEST_FILE_NAME)
    }

    /// Write the manifest to permanent storage
    pub fn save(&self) -> anyhow::Result<()> {
        let manifest_string = self.to_string()?;
        let manifest_path = self.manifest_path();
        std::fs::write(manifest_path, manifest_string)
            .map_err(|e| ManifestError::CannotSaveManifest(e.to_string()))?;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("Manifest file not found at {0}")]
    MissingManifest(String),
    #[error("Could not save manifest file: {0}.")]
    CannotSaveManifest(String),
    #[error("Could not parse manifest because {0}.")]
    TomlParseError(String),
    #[error("Dependency version must be a string. Package name: {0}.")]
    DependencyVersionMustBeString(String),
    #[error("Package must have version that follows semantic versioning. {0}")]
    SemVerError(String),
    #[error("There was an error validating the manifest: {0}")]
    ValidationError(ValidationError),
}

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error(
        "missing ABI field on module {0} used by command {1}; an ABI of `wasi` or `emscripten` is required",
    )]
    MissingABI(String, String),
    #[error("missing module {0} in manifest used by command {1}")]
    MissingModuleForCommand(String, String),
}

#[cfg(test)]
mod serialization_tests {
    use super::*;
    use toml::toml;

    #[test]
    fn get_manifest() {
        let wapm_toml = toml! {
            [package]
            name = "test"
            version = "1.0.0"
            repository = "test.git"
            homepage = "test.com"
            description = "The best package."
        };
        let manifest: Manifest = wapm_toml.try_into().unwrap();
        assert!(!manifest.package.disable_command_rename);
    }
}

#[cfg(test)]
mod command_tests {
    use super::*;
    use toml::toml;

    #[test]
    fn get_commands() {
        let wapm_toml = toml! {
            [package]
            name = "test"
            version = "1.0.0"
            repository = "test.git"
            homepage = "test.com"
            description = "The best package."
            [[module]]
            name = "test-pkg"
            module = "target.wasm"
            source = "source.wasm"
            description = "description"
            interfaces = {"wasi" = "0.0.0-unstable"}
            [[command]]
            name = "foo"
            module = "test"
            [[command]]
            name = "baz"
            module = "test"
            main_args = "$@"
        };
        let manifest: Manifest = wapm_toml.try_into().unwrap();
        let commands = &manifest.command.unwrap();
        assert_eq!(2, commands.len());
    }
}

#[cfg(test)]
mod dependency_tests {
    use super::*;
    use std::{fs::File, io::Write};
    use toml::toml;

    #[test]
    fn add_new_dependency() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let tmp_dir_path: &std::path::Path = tmp_dir.as_ref();
        let manifest_path = tmp_dir_path.join(MANIFEST_FILE_NAME);
        let mut file = File::create(manifest_path).unwrap();
        let wapm_toml = toml! {
            [package]
            name = "_/test"
            version = "1.0.0"
            description = "description"
            [[module]]
            name = "test"
            source = "test.wasm"
            interfaces = {}
        };
        let toml_string = toml::to_string(&wapm_toml).unwrap();
        file.write_all(toml_string.as_bytes()).unwrap();
        let mut manifest = Manifest::find_in_directory(tmp_dir).unwrap();

        let dependency_name = "dep_pkg";
        let dependency_version = semver::Version::new(0, 1, 0);

        manifest.add_dependency(dependency_name.to_string(), dependency_version.to_string());
        assert_eq!(1, manifest.dependencies.as_ref().unwrap().len());

        // adding the same dependency twice changes nothing
        manifest.add_dependency(dependency_name.to_string(), dependency_version.to_string());
        assert_eq!(1, manifest.dependencies.as_ref().unwrap().len());

        // adding a second different dependency will increase the count
        let dependency_name_2 = "dep_pkg_2";
        let dependency_version_2 = semver::Version::new(0, 2, 0);
        manifest.add_dependency(
            dependency_name_2.to_string(),
            dependency_version_2.to_string(),
        );
        assert_eq!(2, manifest.dependencies.as_ref().unwrap().len());
    }
}

#[cfg(test)]
mod manifest_tests {
    use std::fmt::Debug;

    use serde::{de::DeserializeOwned, Deserialize};

    use super::*;

    #[test]
    fn interface_test() {
        let manifest_str = r#"
[package]
name = "test"
version = "0.0.0"
description = "This is a test package"
license = "MIT"

[[module]]
name = "mod"
source = "target/wasm32-wasi/release/mod.wasm"
interfaces = {"wasi" = "0.0.0-unstable"}

[[module]]
name = "mod-with-exports"
source = "target/wasm32-wasi/release/mod-with-exports.wasm"
bindings = { wit-exports = "exports.wit", wit-bindgen = "0.0.0" }

[[command]]
name = "command"
module = "mod"
"#;
        let manifest: Manifest = Manifest::parse(manifest_str).unwrap();
        let modules = manifest.module.as_deref().unwrap();
        assert_eq!(
            modules[0].interfaces.as_ref().unwrap().get("wasi"),
            Some(&"0.0.0-unstable".to_string())
        );

        assert_eq!(
            modules[1],
            Module {
                name: "mod-with-exports".to_string(),
                source: PathBuf::from("target/wasm32-wasi/release/mod-with-exports.wasm"),
                abi: Abi::None,
                kind: None,
                interfaces: None,
                bindings: Some(Bindings::Wit(WitBindings {
                    wit_exports: PathBuf::from("exports.wit"),
                    wit_bindgen: "0.0.0".parse().unwrap()
                })),
            },
        );
    }

    #[test]
    fn parse_wit_bindings() {
        let table = toml::toml! {
            name = "..."
            source = "..."
            bindings = { wit-bindgen = "0.1.0", wit-exports = "./file.wit" }
        };

        let module = Module::deserialize(table).unwrap();

        assert_eq!(
            module.bindings.as_ref().unwrap(),
            &Bindings::Wit(WitBindings {
                wit_bindgen: "0.1.0".parse().unwrap(),
                wit_exports: PathBuf::from("./file.wit"),
            }),
        );
        assert_round_trippable(&module);
    }

    #[test]
    fn parse_wai_bindings() {
        let table = toml::toml! {
            name = "..."
            source = "..."
            bindings = { wai-version = "0.1.0", exports = "./file.wai", imports = ["a.wai", "../b.wai"] }
        };

        let module = Module::deserialize(table).unwrap();

        assert_eq!(
            module.bindings.as_ref().unwrap(),
            &Bindings::Wai(WaiBindings {
                wai_version: "0.1.0".parse().unwrap(),
                exports: Some(PathBuf::from("./file.wai")),
                imports: vec![PathBuf::from("a.wai"), PathBuf::from("../b.wai")],
            }),
        );
        assert_round_trippable(&module);
    }

    #[track_caller]
    fn assert_round_trippable<T>(value: &T)
    where
        T: Serialize + DeserializeOwned + PartialEq + Debug,
    {
        let repr = toml::to_string(value).unwrap();
        let round_tripped: T = toml::from_str(&repr).unwrap();
        assert_eq!(
            round_tripped, *value,
            "The value should convert to/from TOML losslessly"
        );
    }

    #[test]
    fn imports_and_exports_are_optional_with_wai() {
        let table = toml::toml! {
            name = "..."
            source = "..."
            bindings = { wai-version = "0.1.0" }
        };

        let module = Module::deserialize(table).unwrap();

        assert_eq!(
            module.bindings.as_ref().unwrap(),
            &Bindings::Wai(WaiBindings {
                wai_version: "0.1.0".parse().unwrap(),
                exports: None,
                imports: Vec::new(),
            }),
        );
        assert_round_trippable(&module);
    }

    #[test]
    fn ambiguous_bindings_table() {
        let table = toml::toml! {
            wai-version = "0.2.0"
            wit-bindgen = "0.1.0"
        };

        let err = Bindings::deserialize(table).unwrap_err();

        assert_eq!(
            err.to_string(),
            "expected one of \"wit-bindgen\" or \"wai-version\" to be provided, but not both"
        );
    }

    #[test]
    fn bindings_table_that_is_neither_wit_nor_wai() {
        let table = toml::toml! {
            wai-bindgen = "lol, this should have been wai-version"
            exports = "./file.wai"
        };

        let err = Bindings::deserialize(table).unwrap_err();

        assert_eq!(
            err.to_string(),
            "expected one of \"wit-bindgen\" or \"wai-version\" to be provided, but not both"
        );
    }

    #[test]
    fn command_v2_isnt_ambiguous_with_command_v1() {
        let src = r#"
[package]
name = "hotg-ai/sine"
version = "0.12.0"
description = "sine"

[dependencies]
"hotg-ai/train_test_split" = "0.12.1"
"hotg-ai/elastic_net" = "0.12.1"

[[module]] # This is the same as atoms
name = "sine"
kind = "tensorflow-SavedModel" # It can also be "wasm" (default)
source = "models/sine"

[[command]]
name = "run"
runner = "rune"
module = "sine"
annotations = { file = "Runefile.yml", kind = "yaml" }
"#;

        let manifest: Manifest = toml::from_str(src).unwrap();

        let commands = &manifest.command.as_deref().unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(
            commands[0],
            Command::V2(CommandV2 {
                name: "run".into(),
                module: "sine".into(),
                runner: "rune".into(),
                annotations: Some(CommandAnnotations::File(FileCommandAnnotations {
                    file: "Runefile.yml".into(),
                    kind: FileKind::Yaml,
                }))
            })
        );
    }
}

#[test]
fn test_package_name_parse() {
    assert_eq!(
        PackageName::parse("hello").unwrap_err(),
        "no / in package name"
    );
    assert_eq!(
        PackageName::parse("hello/test").unwrap(),
        PackageName {
            namespace: "hello".to_string(),
            name: "test".to_string()
        }
    );
}
