pub mod parser;
pub mod scope;
pub mod symbols;

use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    // Declarations
    ClassDeclaration,
    InterfaceDeclaration,
    ObjectDeclaration,
    CompanionObjectDeclaration,
    FunctionDeclaration,
    PropertyDeclaration,
    EnumEntryDeclaration,
    TypeAliasDeclaration,
    ParameterDeclaration,
    ExtensionFunctionDeclaration,
    // References
    TypeReference,
    CallSite,
    PropertyReference,
    Import,
    ExtensionFunctionCall,
    PackageDeclaration,
}

impl SymbolKind {
    pub fn is_declaration(&self) -> bool {
        matches!(
            self,
            SymbolKind::ClassDeclaration
                | SymbolKind::InterfaceDeclaration
                | SymbolKind::ObjectDeclaration
                | SymbolKind::CompanionObjectDeclaration
                | SymbolKind::FunctionDeclaration
                | SymbolKind::PropertyDeclaration
                | SymbolKind::EnumEntryDeclaration
                | SymbolKind::TypeAliasDeclaration
                | SymbolKind::ParameterDeclaration
                | SymbolKind::ExtensionFunctionDeclaration
        )
    }

    pub fn is_reference(&self) -> bool {
        !self.is_declaration() && !matches!(self, SymbolKind::PackageDeclaration | SymbolKind::Import)
    }
}

#[derive(Debug, Clone)]
pub struct SymbolOccurrence {
    pub name: String,
    pub fqn: Option<String>,
    pub kind: SymbolKind,
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
    pub byte_range: std::ops::Range<usize>,
    pub receiver_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ImportInfo {
    pub path: String,
    pub alias: Option<String>,
    pub is_wildcard: bool,
    pub line: usize,
    pub column: usize,
    pub byte_range: std::ops::Range<usize>,
}

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub path: PathBuf,
    pub package: Option<String>,
    pub imports: Vec<ImportInfo>,
}

#[derive(Debug, Default)]
pub struct SymbolIndex {
    pub by_name: HashMap<String, Vec<SymbolOccurrence>>,
    pub by_fqn: HashMap<String, Vec<SymbolOccurrence>>,
    pub files: HashMap<PathBuf, FileInfo>,
    pub type_aliases: HashMap<String, String>,
}

impl SymbolIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_occurrence(&mut self, occ: SymbolOccurrence) {
        let name = occ.name.clone();
        if let Some(ref fqn) = occ.fqn {
            self.by_fqn.entry(fqn.clone()).or_default().push(occ.clone());
        }
        self.by_name.entry(name).or_default().push(occ);
    }

    pub fn add_file_info(&mut self, info: FileInfo) {
        self.files.insert(info.path.clone(), info);
    }

    pub fn clear(&mut self) {
        self.by_name.clear();
        self.by_fqn.clear();
        self.files.clear();
        self.type_aliases.clear();
    }

    pub fn stats(&self) -> IndexStats {
        IndexStats {
            files: self.files.len(),
            symbols_by_name: self.by_name.len(),
            symbols_by_fqn: self.by_fqn.len(),
            total_occurrences: self.by_name.values().map(|v| v.len()).sum(),
            type_aliases: self.type_aliases.len(),
        }
    }
}

#[derive(Debug)]
pub struct IndexStats {
    pub files: usize,
    pub symbols_by_name: usize,
    pub symbols_by_fqn: usize,
    pub total_occurrences: usize,
    pub type_aliases: usize,
}

impl std::fmt::Display for IndexStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Indexed {} files: {} unique names, {} FQNs, {} total occurrences, {} type aliases",
            self.files, self.symbols_by_name, self.symbols_by_fqn, self.total_occurrences, self.type_aliases
        )
    }
}
