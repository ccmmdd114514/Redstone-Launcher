//! arguments 规则求值器：启动成败的分水岭。
//! 支持两种元素形态（纯字符串 / 带 rules 的条件项）、rules 求值、变量展开、
//! 无值变量整条丢弃。

use crate::meta::{ArgElement, ArgValue, Rule};
use std::collections::HashMap;

/// 启动上下文：所有 ${变量} 的取值来源。
pub struct Ctx {
    pub natives_dir: String,
    pub classpath: String,
    pub player_name: String,
    pub uuid: String,
    pub access_token: String,
    pub game_dir: String,
    pub assets_root: String,
    pub assets_index_name: String,
    pub version_name: String,
    pub version_type: String,
}

impl Ctx {
    fn var(&self, name: &str) -> Option<String> {
        Some(match name {
            "natives_directory" => self.natives_dir.clone(),
            "classpath" => self.classpath.clone(),
            "auth_player_name" => self.player_name.clone(),
            "auth_uuid" => self.uuid.clone(),
            "auth_access_token" => self.access_token.clone(),
            "game_directory" => self.game_dir.clone(),
            "assets_root" => self.assets_root.clone(),
            "assets_index_name" => self.assets_index_name.clone(),
            "version_name" => self.version_name.clone(),
            "version_type" => self.version_type.clone(),
            "launcher_name" => crate::paths::LAUNCHER_NAME.to_string(),
            "launcher_version" => crate::paths::LAUNCHER_VERSION.to_string(),
            "user_type" => "legacy".to_string(),
            "resolution_width" => "854".to_string(),
            "resolution_height" => "480".to_string(),
            // 离线模式无值：${clientid} / ${auth_xuid} / ${quickPlayPath} / ${user_properties}
            _ => return None,
        })
    }
}

/// 求值一条 arguments 列表。features 为特性开关（离线模式全为 false）。
pub fn eval(elements: &[ArgElement], ctx: &Ctx, features: &HashMap<String, bool>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for el in elements {
        match el {
            ArgElement::Plain(s) => match expand(s, ctx) {
                Some(v) => out.push(v),
                None => {
                    eprintln!("  [丢弃] 无值变量：{s}");
                    pop_dangling_flag(&mut out);
                }
            },
            ArgElement::Conditional { rules, value } => {
                if !rules_allow(rules, features) {
                    continue;
                }
                match value {
                    ArgValue::One(s) => match expand(s, ctx) {
                        Some(v) => out.push(v),
                        None => {
                            eprintln!("  [丢弃] 无值变量：{s}");
                            pop_dangling_flag(&mut out);
                        }
                    },
                    ArgValue::Many(vs) => {
                        let mut tmp = Vec::new();
                        let mut missing = false;
                        for s in vs {
                            match expand(s, ctx) {
                                Some(v) => tmp.push(v),
                                None => missing = true,
                            }
                        }
                        if missing {
                            eprintln!("  [丢弃] 整组含无值变量：{vs:?}");
                            continue;
                        }
                        out.extend(tmp);
                    }
                }
            }
        }
    }
    out
}

/// 值被丢弃后，把它前面的 --flag 也一起丢掉，避免 `--clientId` 悬空。
fn pop_dangling_flag(out: &mut Vec<String>) {
    if let Some(last) = out.last() {
        if last.starts_with("--") && !last.contains('=') {
            out.pop();
        }
    }
}

/// rules 求值：
/// - 列表中存在 allow 规则时，默认「不应用」（命中才放行）；
/// - 列表全是 disallow 规则时，默认「应用」（没人禁止就是允许）；
/// - 最后一条命中的规则决定结果。
pub fn rules_allow(rules: &[Rule], features: &HashMap<String, bool>) -> bool {
    let has_allow = rules.iter().any(|r| r.action == "allow");
    let mut allowed = !has_allow;
    for r in rules {
        if rule_matches(r, features) {
            allowed = r.action == "allow";
        }
    }
    allowed
}

fn rule_matches(rule: &Rule, features: &HashMap<String, bool>) -> bool {
    if let Some(os) = &rule.os {
        if let Some(name) = &os.name {
            if name != "windows" {
                return false;
            }
        }
        if let Some(arch) = &os.arch {
            // 本机为 x86_64；version.json 里的 "x86" 指 32 位，不应命中。
            if arch != "x86_64" && arch != "x64" && arch != "amd64" {
                return false;
            }
        }
    }
    if let Some(f) = &rule.features {
        for (key, want) in f {
            let have = features.get(key).copied().unwrap_or(false);
            if have != *want {
                return false;
            }
        }
    }
    true
}

/// 展开 ${变量}。遇到无值变量返回 None（由调用方决定丢弃策略）。
fn expand(s: &str, ctx: &Ctx) -> Option<String> {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find("${") {
        let end = match rest[start..].find('}') {
            Some(e) => start + e,
            None => return None,
        };
        out.push_str(&rest[..start]);
        let name = &rest[start + 2..end];
        match ctx.var(name) {
            Some(v) => out.push_str(&v),
            None => return None,
        }
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> Ctx {
        Ctx {
            natives_dir: r"C:\natives".into(),
            classpath: "a.jar;b.jar".into(),
            player_name: "阿强".into(),
            uuid: "u-1".into(),
            access_token: "t-1".into(),
            game_dir: r"C:\game".into(),
            assets_root: r"C:\assets".into(),
            assets_index_name: "26".into(),
            version_name: "1.21.8".into(),
            version_type: "release".into(),
        }
    }

    #[test]
    fn expand_and_drop() {
        let c = ctx();
        assert_eq!(expand("--username ${auth_player_name}", &c).unwrap(), "--username 阿强");
        assert!(expand("--clientId ${clientid}", &c).is_none());
    }

    #[test]
    fn rule_defaults() {
        let allow_win = Rule {
            action: "allow".into(),
            os: Some(crate::meta::OsRule {
                name: Some("windows".into()),
                arch: None,
                version: None,
            }),
            features: None,
        };
        let disallow_mac = Rule {
            action: "disallow".into(),
            os: Some(crate::meta::OsRule {
                name: Some("osx".into()),
                arch: None,
                version: None,
            }),
            features: None,
        };
        let f = HashMap::new();
        assert!(rules_allow(&[allow_win], &f));
        assert!(rules_allow(&[disallow_mac], &f));
        assert!(!rules_allow(
            &[Rule {
                action: "disallow".into(),
                os: Some(crate::meta::OsRule {
                    name: Some("windows".into()),
                    arch: None,
                    version: None
                }),
                features: None
            }],
            &f
        ));
    }
}
