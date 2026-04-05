// SubscribeRequest validation against FilterRules.
//
// SubscribeRequest protobuf field numbers (from Yellowstone geyser.proto):
//   1  = accounts         (map<string, SubscribeRequestFilterAccounts>)
//   3  = transactions     (map<string, SubscribeRequestFilterTransactions>)
//   4  = blocks           (map<string, SubscribeRequestFilterBlocks>)
//   5  = blocks_meta      (map<string, SubscribeRequestFilterBlocksMeta>)
//   7  = accounts_data_slice (repeated)
//   8  = entry            (map<string, SubscribeRequestFilterEntry>)
//   10 = transactions_status (map<string, SubscribeRequestFilterTransactions>)

use crate::protobuf::{
    count_repeated_strings, extract_len_fields, extract_map_values, has_rejected,
    read_bool_field,
};
use crate::rules::{FilterRules, TransactionsLimits};

pub fn validate_subscribe_request(
    rules: &FilterRules,
    proto_buf: &[u8],
) -> Result<(), String> {
    validate_accounts(rules, proto_buf)?;
    validate_tx_filters(proto_buf, 3, &rules.transactions, "transactions")?;
    validate_blocks(rules, proto_buf)?;
    validate_map_count(proto_buf, 5, rules.blocks_meta_max, "blocks_meta")?;
    validate_tx_filters(
        proto_buf,
        10,
        &rules.transactions_status,
        "transactions_status",
    )?;
    validate_map_count(proto_buf, 8, rules.entry_max, "entry")?;
    validate_data_slices(rules, proto_buf)?;
    Ok(())
}

fn validate_accounts(rules: &FilterRules, proto_buf: &[u8]) -> Result<(), String> {
    for filter_buf in &extract_map_values(proto_buf, 1) {
        let accounts = count_repeated_strings(filter_buf, 2);
        if accounts.len() as i64 > rules.accounts.account_max {
            return Err(format!(
                "accounts filter: too many accounts ({} > {})",
                accounts.len(),
                rules.accounts.account_max
            ));
        }
        if let Some(rejected) = has_rejected(&accounts, &rules.accounts.account_reject) {
            return Err(format!(
                "accounts filter: account {rejected} is not allowed"
            ));
        }
        let owners = count_repeated_strings(filter_buf, 3);
        if owners.len() as i64 > rules.accounts.owner_max {
            return Err(format!(
                "accounts filter: too many owners ({} > {})",
                owners.len(),
                rules.accounts.owner_max
            ));
        }
        if let Some(rejected) = has_rejected(&owners, &rules.accounts.owner_reject) {
            return Err(format!(
                "accounts filter: owner {rejected} is not allowed"
            ));
        }
    }
    Ok(())
}

fn validate_tx_filters(
    proto_buf: &[u8],
    map_field: u32,
    limits: &TransactionsLimits,
    label: &str,
) -> Result<(), String> {
    for filter_buf in &extract_map_values(proto_buf, map_field) {
        let includes = count_repeated_strings(filter_buf, 3);
        if includes.len() as i64 > limits.account_include_max {
            return Err(format!(
                "{label} filter: too many account_include ({} > {})",
                includes.len(),
                limits.account_include_max
            ));
        }
        if let Some(rejected) = has_rejected(&includes, &limits.account_include_reject) {
            return Err(format!(
                "{label} filter: account {rejected} is not allowed in account_include"
            ));
        }
        let excludes = count_repeated_strings(filter_buf, 4);
        if excludes.len() as i64 > limits.account_exclude_max {
            return Err(format!(
                "{label} filter: too many account_exclude ({} > {})",
                excludes.len(),
                limits.account_exclude_max
            ));
        }
        let required = count_repeated_strings(filter_buf, 6);
        if required.len() as i64 > limits.account_required_max {
            return Err(format!(
                "{label} filter: too many account_required ({} > {})",
                required.len(),
                limits.account_required_max
            ));
        }
    }
    Ok(())
}

fn validate_blocks(rules: &FilterRules, proto_buf: &[u8]) -> Result<(), String> {
    for filter_buf in &extract_map_values(proto_buf, 4) {
        let includes = count_repeated_strings(filter_buf, 1);
        if includes.len() as i64 > rules.blocks.account_include_max {
            return Err(format!(
                "blocks filter: too many account_include ({} > {})",
                includes.len(),
                rules.blocks.account_include_max
            ));
        }
        if let Some(rejected) = has_rejected(&includes, &rules.blocks.account_include_reject) {
            return Err(format!(
                "blocks filter: account {rejected} is not allowed in account_include"
            ));
        }
        if let Some(true) = read_bool_field(filter_buf, 2) {
            if !rules.blocks.include_transactions {
                return Err("blocks filter: include_transactions is not allowed".to_string());
            }
        }
        if let Some(true) = read_bool_field(filter_buf, 3) {
            if !rules.blocks.include_accounts {
                return Err("blocks filter: include_accounts is not allowed".to_string());
            }
        }
        if let Some(true) = read_bool_field(filter_buf, 4) {
            if !rules.blocks.include_entries {
                return Err("blocks filter: include_entries is not allowed".to_string());
            }
        }
    }
    Ok(())
}

fn validate_map_count(
    proto_buf: &[u8],
    map_field: u32,
    max: i64,
    label: &str,
) -> Result<(), String> {
    if max <= 0 {
        return Ok(()); // 0 = unlimited
    }
    let count = extract_len_fields(proto_buf, map_field).len();
    if count as i64 > max {
        return Err(format!("{label}: too many filters ({count} > {max})"));
    }
    Ok(())
}

fn validate_data_slices(rules: &FilterRules, proto_buf: &[u8]) -> Result<(), String> {
    let slices = extract_len_fields(proto_buf, 7);
    if slices.len() as i64 > rules.accounts.data_slice_max {
        return Err(format!(
            "accounts_data_slice: too many slices ({} > {})",
            slices.len(),
            rules.accounts.data_slice_max
        ));
    }
    Ok(())
}
