// Per-IP FilterRules loaded from TOML files in rules_dir.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use toml::Value;

type TomlTable = toml::map::Map<String, Value>;

#[derive(Debug, Clone)]
pub struct AccountsLimits {
    pub account_max: i64,
    pub owner_max: i64,
    pub data_slice_max: i64,
    pub account_reject: HashSet<String>,
    pub owner_reject: HashSet<String>,
}

#[derive(Debug, Clone)]
pub struct TransactionsLimits {
    pub account_include_max: i64,
    pub account_exclude_max: i64,
    pub account_required_max: i64,
    pub account_include_reject: HashSet<String>,
}

#[derive(Debug, Clone)]
pub struct BlocksLimits {
    pub account_include_max: i64,
    pub include_accounts: bool,
    pub include_entries: bool,
    pub include_transactions: bool,
    pub account_include_reject: HashSet<String>,
}

#[derive(Debug, Clone)]
pub struct FilterRules {
    pub max_connections: i64,
    pub accounts: AccountsLimits,
    pub transactions: TransactionsLimits,
    pub blocks: BlocksLimits,
    pub transactions_status: TransactionsLimits,
    pub blocks_meta_max: i64,
    pub entry_max: i64,
    /// Maximum number of SubscribeRequest updates per stream within
    /// `churn_window_secs`. 0 = no limit.
    pub max_updates_per_window: i64,
    /// Time window (seconds) for churn protection. Default 10s.
    pub churn_window_secs: i64,
}

fn get_sub_table<'a>(conf: &'a TomlTable, key: &str) -> Option<&'a TomlTable> {
    conf.get(key).and_then(|v| v.as_table())
}

fn sub_int(table: Option<&TomlTable>, key: &str, default: i64) -> i64 {
    table
        .and_then(|t| t.get(key))
        .and_then(|v| v.as_integer())
        .unwrap_or(default)
}

fn sub_bool(table: Option<&TomlTable>, key: &str, default: bool) -> bool {
    table
        .and_then(|t| t.get(key))
        .and_then(|v| v.as_bool())
        .unwrap_or(default)
}

fn sub_str_slice(table: Option<&TomlTable>, key: &str) -> HashSet<String> {
    table
        .and_then(|t| t.get(key))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item.as_str())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

impl FilterRules {
    pub fn from_toml(conf: &TomlTable) -> Self {
        let acc = get_sub_table(conf, "accounts");
        let tx = get_sub_table(conf, "transactions");
        let blk = get_sub_table(conf, "blocks");
        let tx_st = get_sub_table(conf, "transactions_status");

        let max_conn = conf
            .get("max_connections")
            .and_then(|v| v.as_integer())
            .unwrap_or(0);

        Self {
            max_connections: max_conn,
            accounts: AccountsLimits {
                account_max: sub_int(acc, "account_max", 100),
                owner_max: sub_int(acc, "owner_max", 20),
                data_slice_max: sub_int(acc, "data_slice_max", 2),
                account_reject: sub_str_slice(acc, "account_reject"),
                owner_reject: sub_str_slice(acc, "owner_reject"),
            },
            transactions: TransactionsLimits {
                account_include_max: sub_int(tx, "account_include_max", 100),
                account_exclude_max: sub_int(tx, "account_exclude_max", 100),
                account_required_max: sub_int(tx, "account_required_max", 100),
                account_include_reject: sub_str_slice(tx, "account_include_reject"),
            },
            blocks: BlocksLimits {
                account_include_max: sub_int(blk, "account_include_max", 20),
                include_accounts: sub_bool(blk, "include_accounts", false),
                include_entries: sub_bool(blk, "include_entries", false),
                include_transactions: sub_bool(blk, "include_transactions", true),
                account_include_reject: sub_str_slice(blk, "account_include_reject"),
            },
            transactions_status: TransactionsLimits {
                account_include_max: sub_int(tx_st, "account_include_max", 20),
                account_exclude_max: sub_int(tx_st, "account_exclude_max", 20),
                account_required_max: sub_int(tx_st, "account_required_max", 20),
                account_include_reject: sub_str_slice(tx_st, "account_include_reject"),
            },
            blocks_meta_max: get_sub_table(conf, "blocks_meta")
                .map(|t| sub_int(Some(t), "max", 0))
                .unwrap_or(0),
            entry_max: get_sub_table(conf, "entry")
                .map(|t| sub_int(Some(t), "max", 0))
                .unwrap_or(0),
            max_updates_per_window: get_sub_table(conf, "churn")
                .map(|t| sub_int(Some(t), "max_updates", 0))
                .unwrap_or(0),
            churn_window_secs: get_sub_table(conf, "churn")
                .map(|t| sub_int(Some(t), "window_secs", 10))
                .unwrap_or(10),
        }
    }
}

/// Load all <IP>.toml files from a directory into a HashMap<IP, Arc<FilterRules>>.
pub fn load_rules_from_dir(
    dir: &Path,
) -> Result<HashMap<String, Arc<FilterRules>>, String> {
    let mut ip_rules = HashMap::new();

    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("failed to read rules_dir {}: {e}", dir.display()))?;

    for entry in entries {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }

        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("failed to read {}: {e}", path.display()))?;

        let table: TomlTable = toml::from_str(&content)
            .map_err(|e| format!("failed to parse {}: {e}", path.display()))?;

        let rules = Arc::new(FilterRules::from_toml(&table));

        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        if !stem.is_empty() {
            ip_rules.insert(stem, rules);
        }
    }

    Ok(ip_rules)
}
