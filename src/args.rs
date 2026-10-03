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

    fn os_rule(arch: &str) -> Rule {
        Rule {
            action: "allow".into(),
            os: Some(crate::meta::OsRule {
                name: None,
                arch: Some(arch.into()),
                version: None,
            }),
            features: None,
        }
    }

    #[test]
    fn expand_mixed_text() {
        let c = ctx();
        assert_eq!(expand("-Dfoo=${auth_uuid}-bar", &c).unwrap(), "-Dfoo=u-1-bar");
    }

    #[test]
    fn expand_rejects_unclosed_placeholder() {
        let c = ctx();
        assert!(
            expand("--x=${broken", &c).is_none(),
            "占位符没闭合应判为无值，而不是原样塞给 java"
        );
    }

    #[test]
    fn dangling_flag_dropped_only_when_plain() {
        let mut out: Vec<String> = vec!["--clientId".into()];
        pop_dangling_flag(&mut out);
        assert!(out.is_empty(), "值被丢掉的瞬间，前面的 --clientId 也得跟着走");

        let mut with_eq: Vec<String> = vec!["--foo=bar".into()];
        pop_dangling_flag(&mut with_eq);
        assert_eq!(with_eq, vec!["--foo=bar"], "含 = 的不是悬空标志，不该动它");

        let mut empty: Vec<String> = vec![];
        pop_dangling_flag(&mut empty);
        assert!(empty.is_empty(), "空列表不能 panic");
    }

    fn feature_rule(want: bool) -> Rule {
        Rule {
            action: "allow".into(),
            os: None,
            features: Some(HashMap::from([("is_demo_user".to_string(), want)])),
        }
    }

    #[test]
    fn features_rule() {
        let off = HashMap::new();
        assert!(
            !rules_allow(&[feature_rule(true)], &off),
            "特性没开就不该命中"
        );

        let mut on = HashMap::new();
        on.insert("is_demo_user".to_string(), true);
        assert!(
            rules_allow(&[feature_rule(true)], &on),
            "特性开了就该命中"
        );
    }

    #[test]
    fn arch_x86_is_not_this_machine() {
        // version.json 里的 "x86" 指 32 位；本机是 x86_64，不该命中这条规则
        let f = HashMap::new();
        assert!(!rules_allow(&[os_rule("x86")], &f));
        assert!(rules_allow(&[os_rule("x86_64")], &f));
        assert!(rules_allow(&[os_rule("amd64")], &f));
    }

    #[test]
    fn eval_drops_group_with_missing_var() {
        // 一组参数里只要有一个变量无值，整组都得丢——留半截比全丢更糟，
        // java 会因为参数错位直接报看不懂的错误。
        let c = ctx();
        let els = vec![ArgElement::Conditional {
            rules: vec![],
            value: ArgValue::Many(vec![
                "--a".into(),
                "--clientId".into(),
                "${clientid}".into(),
            ]),
        }];
        assert!(eval(&els, &c, &HashMap::new()).is_empty());
    }

    #[test]
    fn eval_keeps_plain_args() {
        let c = ctx();
        let els = vec![
            ArgElement::Plain("--username".into()),
            ArgElement::Plain("${auth_player_name}".into()),
            ArgElement::Plain("-Dfile.encoding=UTF-8".into()),
        ];
        assert_eq!(
            eval(&els, &c, &HashMap::new()),
            vec!["--username", "阿强", "-Dfile.encoding=UTF-8"]
        );
    }

    #[test]
    fn eval_respects_disallow_on_windows() {
        let c = ctx();
        let els = vec![ArgElement::Conditional {
            rules: vec![Rule {
                action: "disallow".into(),
                os: Some(crate::meta::OsRule {
                    name: Some("windows".into()),
                    arch: None,
                    version: None,
                }),
                features: None,
            }],
            value: ArgValue::One("--mac-only".into()),
        }];
        assert!(eval(&els, &c, &HashMap::new()).is_empty());
    }
}
