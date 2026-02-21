pub mod helpers;
pub mod encode;
pub mod convert;
pub mod text;
pub mod time;
pub mod gen;
pub mod regex;
pub mod cron;
pub mod crypto;
pub mod format;
pub mod network;
pub mod dns;
pub mod env;
pub mod port;
pub mod file;
pub mod image;
pub mod hosts;
pub mod manuals;
pub mod settings;
pub mod hotkey;
pub mod jwt;
pub mod schema;
pub mod mybatis;
pub mod nginx;
pub mod snippets;

use serde_json::Value;

pub fn execute_tool(domain: &str, action: &str, payload: &Value) -> Result<Value, String> {
    match domain {
        "encode"   => encode::execute(action, payload),
        "convert"  => convert::execute(action, payload),
        "text"     => text::execute(action, payload),
        "time"     => time::execute(action, payload),
        "gen"      => gen::execute(action, payload),
        "regex"    => regex::execute(action, payload),
        "cron"     => cron::execute(action, payload),
        "crypto"   => crypto::execute(action, payload),
        "format"   => format::execute(action, payload),
        "network"  => network::execute(action, payload),
        "dns"      => dns::execute(action, payload),
        "env"      => env::execute(action, payload),
        "port"     => port::execute(action, payload),
        "file"     => file::execute(action, payload),
        "image"    => image::execute(action, payload),
        "hosts"    => hosts::execute(action, payload),
        "manuals"  => manuals::execute(action, payload),
        "settings" => settings::execute(action, payload),
        "hotkey"   => hotkey::execute(action, payload),
        "jwt"      => jwt::execute(action, payload),
        "schema"   => schema::execute(action, payload),
        "mybatis"  => mybatis::execute(action, payload),
        "nginx"    => nginx::execute(action, payload),
        "snippets" => snippets::execute(action, payload),
        _ => Err(format!("unsupported command: {domain}.{action}")),
    }
}
