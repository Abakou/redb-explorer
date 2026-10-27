//! Reading arbitrary redb files.
//!
//! redb tables are typed and the API needs the Rust types to read them. We probe each
//! table with a fake type, read the real type names from the TableTypeMismatch error,
//! then reopen it with the matching Rust types (see `read_table`).

use redb::{
    Key, MultimapTableHandle, ReadOnlyDatabase, ReadTransaction, ReadableDatabase,
    ReadableTable, ReadableTableMetadata, TableDefinition, TableError, TableHandle, TypeName,
    Value,
};
use std::cmp::Ordering;
use std::path::{Path, PathBuf};

pub const MAX_ROWS: usize = 200_000;

pub type Rows = Vec<(String, String)>;

pub struct Db {
    pub path: PathBuf,
    pub db: ReadOnlyDatabase,
    pub tables: Vec<TableInfo>,
    pub file_size: u64,
}

impl Db {
    pub fn open(path: &Path) -> Result<Db, String> {
        let db = ReadOnlyDatabase::open(path).map_err(|e| e.to_string())?;
        let txn = db.begin_read().map_err(|e| e.to_string())?;
        let tables = list_tables(&txn)?;
        drop(txn);
        let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        Ok(Db { path: path.to_path_buf(), db, tables, file_size })
    }

    pub fn file_name(&self) -> String {
        self.path.file_name().map_or_else(|| self.path.display().to_string(), |n| n.to_string_lossy().into())
    }

    pub fn table(&self, name: &str) -> Option<&TableInfo> {
        self.tables.iter().find(|t| t.name == name)
    }

    pub fn read(&self, table: &TableInfo) -> Result<(Rows, bool), String> {
        if table.multimap {
            return Err("Tables multimap non supportées.".into());
        }
        let txn = self.db.begin_read().map_err(|e| e.to_string())?;
        read_table(&txn, table)
    }
}

pub struct TableInfo {
    pub name: String,
    pub key_type: String,
    pub value_type: String,
    pub len: u64,
    pub multimap: bool,
    pub stats: Option<Stats>,
}

pub struct Stats {
    pub tree_height: u32,
    pub leaf_pages: u64,
    pub branch_pages: u64,
    pub stored_bytes: u64,
    pub metadata_bytes: u64,
    pub fragmented_bytes: u64,
}

fn list_tables(txn: &ReadTransaction) -> Result<Vec<TableInfo>, String> {
    let mut tables = Vec::new();
    for handle in txn.list_tables().map_err(|e| e.to_string())? {
        let name = handle.name().to_string();
        let (len, stats) = match txn.open_untyped_table(handle) {
            Ok(t) => (
                t.len().unwrap_or(0),
                t.stats().ok().map(|s| Stats {
                    tree_height: s.tree_height(),
                    leaf_pages: s.leaf_pages(),
                    branch_pages: s.branch_pages(),
                    stored_bytes: s.stored_bytes(),
                    metadata_bytes: s.metadata_bytes(),
                    fragmented_bytes: s.fragmented_bytes(),
                }),
            ),
            Err(_) => (0, None),
        };
        let (key_type, value_type) =
            match txn.open_table(TableDefinition::<Probe, Probe>::new(&name)) {
                Err(TableError::TableTypeMismatch { key, value, .. }) => {
                    (key.name().to_string(), value.name().to_string())
                }
                Err(e) => ("?".into(), e.to_string()),
                Ok(_) => ("?".into(), "?".into()),
            };
        tables.push(TableInfo { name, key_type, value_type, len, multimap: false, stats });
    }
    for handle in txn.list_multimap_tables().map_err(|e| e.to_string())? {
        tables.push(TableInfo {
            name: handle.name().to_string(),
            key_type: "?".into(),
            value_type: "?".into(),
            len: 0,
            multimap: true,
            stats: None,
        });
    }
    Ok(tables)
}

// ---------------------------------------------------------------------------
// Type probing and decoding

#[derive(Debug)]
struct Probe;

impl Value for Probe {
    type SelfType<'a> = ();
    type AsBytes<'a> = &'a [u8];
    fn fixed_width() -> Option<usize> {
        None
    }
    fn from_bytes<'a>(_: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
    }
    fn as_bytes<'a, 'b: 'a>(_: &'a ()) -> &'a [u8] {
        &[]
    }
    fn type_name() -> TypeName {
        TypeName::new("redb_explorer::Probe")
    }
}

impl Key for Probe {
    fn compare(a: &[u8], b: &[u8]) -> Ordering {
        a.cmp(b)
    }
}

/// Renders a decoded value as display text.
trait Show: Value {
    fn show<'a>(v: &Self::SelfType<'a>) -> String
    where
        Self: 'a;
}

macro_rules! show_with_to_string {
    ($($t:ty),*) => {$(
        impl Show for $t {
            fn show<'a>(v: &Self::SelfType<'a>) -> String where Self: 'a { v.to_string() }
        }
    )*};
}
show_with_to_string!(String, u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, f32, f64, bool, char);

impl Show for &str {
    fn show<'a>(v: &Self::SelfType<'a>) -> String
    where
        Self: 'a,
    {
        v.to_string()
    }
}

impl Show for &[u8] {
    fn show<'a>(v: &Self::SelfType<'a>) -> String
    where
        Self: 'a,
    {
        bytes_to_text(v)
    }
}

impl Show for () {
    fn show<'a>(_: &Self::SelfType<'a>) -> String
    where
        Self: 'a,
    {
        "()".into()
    }
}

/// Printable UTF-8 is shown as text, anything else as hex.
fn bytes_to_text(b: &[u8]) -> String {
    match std::str::from_utf8(b) {
        Ok(s) if !s.chars().any(|c| c.is_control() && c != '\n' && c != '\t') => s.to_string(),
        _ => format!("0x{}", b.iter().map(|x| format!("{x:02x}")).collect::<String>()),
    }
}

fn collect<K: Key + Show + 'static, V: Value + Show + 'static>(
    txn: &ReadTransaction,
    name: &str,
) -> Result<(Rows, bool), String> {
    let table = txn
        .open_table(TableDefinition::<K, V>::new(name))
        .map_err(|e| e.to_string())?;
    let mut rows = Vec::new();
    for row in table.iter().map_err(|e| e.to_string())? {
        if rows.len() == MAX_ROWS {
            return Ok((rows, true));
        }
        let (k, v) = row.map_err(|e| e.to_string())?;
        rows.push((K::show(&k.value()), V::show(&v.value())));
    }
    Ok((rows, false))
}

// Dispatch on the stored type names. Add types here if your tables use others.
macro_rules! with_value {
    ($k:ty, $v:expr, $txn:expr, $name:expr) => {
        match $v {
            "&str" => collect::<$k, &str>($txn, $name),
            "String" => collect::<$k, String>($txn, $name),
            "&[u8]" => collect::<$k, &[u8]>($txn, $name),
            "u8" => collect::<$k, u8>($txn, $name),
            "u16" => collect::<$k, u16>($txn, $name),
            "u32" => collect::<$k, u32>($txn, $name),
            "u64" => collect::<$k, u64>($txn, $name),
            "u128" => collect::<$k, u128>($txn, $name),
            "i8" => collect::<$k, i8>($txn, $name),
            "i16" => collect::<$k, i16>($txn, $name),
            "i32" => collect::<$k, i32>($txn, $name),
            "i64" => collect::<$k, i64>($txn, $name),
            "i128" => collect::<$k, i128>($txn, $name),
            "f32" => collect::<$k, f32>($txn, $name),
            "f64" => collect::<$k, f64>($txn, $name),
            "bool" => collect::<$k, bool>($txn, $name),
            "char" => collect::<$k, char>($txn, $name),
            "()" => collect::<$k, ()>($txn, $name),
            other => Err(format!("type de valeur non supporté : {other}")),
        }
    };
}

fn read_table(txn: &ReadTransaction, t: &TableInfo) -> Result<(Rows, bool), String> {
    let (name, v) = (t.name.as_str(), t.value_type.as_str());
    match t.key_type.as_str() {
        "&str" => with_value!(&str, v, txn, name),
        "String" => with_value!(String, v, txn, name),
        "&[u8]" => with_value!(&[u8], v, txn, name),
        "u8" => with_value!(u8, v, txn, name),
        "u16" => with_value!(u16, v, txn, name),
        "u32" => with_value!(u32, v, txn, name),
        "u64" => with_value!(u64, v, txn, name),
        "u128" => with_value!(u128, v, txn, name),
        "i32" => with_value!(i32, v, txn, name),
        "i64" => with_value!(i64, v, txn, name),
        "i128" => with_value!(i128, v, txn, name),
        "bool" => with_value!(bool, v, txn, name),
        "char" => with_value!(char, v, txn, name),
        other => Err(format!("type de clé non supporté : {other}")),
    }
}

pub fn is_numeric(type_name: &str) -> bool {
    matches!(
        type_name,
        "u8" | "u16" | "u32" | "u64" | "u128" | "i8" | "i16" | "i32" | "i64" | "i128" | "f32" | "f64"
    )
}
