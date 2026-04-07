use crate::db::models::Language;

pub fn default_languages() -> Vec<Language> {
    vec![
        Language {
            id: 1,
            name: "python3".to_string(),
            display_name: "Python 3".to_string(),
            file_extension: "py".to_string(),
            run_command: "/usr/bin/python3 {file}".to_string(),
            is_enabled: true,
        },
        Language {
            id: 2,
            name: "bash".to_string(),
            display_name: "Bash".to_string(),
            file_extension: "sh".to_string(),
            run_command: "/usr/bin/bash {file}".to_string(),
            is_enabled: true,
        },
        Language {
            id: 3,
            name: "ruby".to_string(),
            display_name: "Ruby".to_string(),
            file_extension: "rb".to_string(),
            run_command: "/usr/bin/ruby {file}".to_string(),
            is_enabled: true,
        },
        Language {
            id: 4,
            name: "perl".to_string(),
            display_name: "Perl".to_string(),
            file_extension: "pl".to_string(),
            run_command: "/usr/bin/perl {file}".to_string(),
            is_enabled: true,
        },
        Language {
            id: 5,
            name: "node".to_string(),
            display_name: "Node.js".to_string(),
            file_extension: "js".to_string(),
            run_command: "/usr/bin/node {file}".to_string(),
            is_enabled: true,
        },
        Language {
            id: 6,
            name: "lua".to_string(),
            display_name: "Lua".to_string(),
            file_extension: "lua".to_string(),
            run_command: "/usr/bin/lua {file}".to_string(),
            is_enabled: true,
        },
    ]
}
