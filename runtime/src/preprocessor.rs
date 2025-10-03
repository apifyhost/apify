use regex::Regex;
use serde_yaml::{Mapping, Value};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use flow::evaluate_expression;  // 引用flow项目的表达式解析函数

pub fn preprocessor(
    phlow: &str,
    base_path: &Path,
    print_phlow: bool,
) -> Result<String, Vec<String>> {
    let (phlow, errors) = preprocessor_directives(phlow, base_path);

    if !errors.is_empty() {
        eprintln!("❌ YAML Transformation Errors:");
        for (i, error) in errors.iter().enumerate() {
            eprintln!("  {}. {}", i + 1, error);
        }
        eprintln!();
        return Err(errors);
    }

    let phlow = preprocessor_eval(&phlow);
    let phlow = preprocessor_modules(&phlow)?;

    if print_phlow {
        println!("");
        println!("#####################################################################");
        println!("# RUNTIME TRANSFORMED");
        println!("#####################################################################");
        println!("{}", phlow);
        println!("#####################################################################");
        println!("");
    }

    Ok(phlow)
}

fn preprocessor_directives(phlow: &str, base_path: &Path) -> (String, Vec<String>) {
    let mut errors = Vec::new();
    let include_block_regex = match Regex::new(r"(?m)^(\s*)!include\s+([^\s]+)(.*)") {
        Ok(re) => re,
        Err(_) => return (phlow.to_string(), errors),
    };
    let include_inline_regex = match Regex::new(r"!include\s+([^\s]+)(.*)") {
        Ok(re) => re,
        Err(_) => return (phlow.to_string(), errors),
    };
    let import_inline_regex = match Regex::new(r"!import\s+(\S+)") {
        Ok(re) => re,
        Err(_) => return (phlow.to_string(), errors),
    };

    let with_block_includes = include_block_regex.replace_all(&phlow, |caps: &regex::Captures| {
        let indent = &caps[1];
        let rel_path = &caps[2];
        let args_str = caps.get(3).map(|m| m.as_str()).unwrap_or("").trim();
        let args = parse_include_args(args_str);
        let full_path = base_path.join(rel_path);
        match process_include_file(&full_path, &args) {
            Ok(json_str) => json_str
                .lines()
                .map(|line| format!("{}{}", indent, line))
                .collect::<Vec<_>>()
                .join("\n"),
            Err(e) => {
                errors.push(format!("Error including file {}: {}", rel_path, e));
                format!("{}<!-- Error including file: {} -->", indent, rel_path)
            }
        }
    });

    let with_inline_includes =
        include_inline_regex.replace_all(&with_block_includes, |caps: &regex::Captures| {
            let rel_path = &caps[1];
            let args_str = caps.get(2).map(|m| m.as_str()).unwrap_or("").trim();
            let args = parse_include_args(args_str);
            let full_path = base_path.join(rel_path);
            match process_include_file(&full_path, &args) {
                Ok(json_str) => json_str,
                Err(e) => {
                    errors.push(format!("Error including file {}: {}", rel_path, e));
                    format!("<!-- Error including file: {} -->", rel_path)
                }
            }
        });

    let result = import_inline_regex
        .replace_all(&with_inline_includes, |caps: &regex::Captures| {
            let rel_path = &caps[1];
            let full_path = base_path.join(rel_path);
            let extension = full_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            match fs::read_to_string(&full_path) {
                Ok(contents) => {
                    let one_line = contents
                        .lines()
                        .map(str::trim)
                        .collect::<Vec<_>>()
                        .join(" ")
                        .replace('"', "\\\"");

                    if extension == "phs" || extension == "rhai" {
                        format!(r#""{{{{ {} }}}}""#, one_line)
                    } else {
                        format!(r#""{}""#, one_line)
                    }
                }
                Err(_) => {
                    errors.push(format!("Error importing file {}: file not found", rel_path));
                    format!("<!-- Error importing file: {} -->", rel_path)
                }
            }
        })
        .to_string();

    (result, errors)
}

fn preprocessor_eval(phlow: &str) -> String {
    let mut result = String::new();
    let mut lines = phlow.lines().peekable();

    while let Some(line) = lines.next() {
        if let Some(pos) = line.find("!phs") {
            let before_eval = &line[..pos];
            let after_eval = if line.len() > pos + 4 {
                line[pos + 4..].trim()
            } else {
                ""
            };
            let indent = " ".repeat(pos);

            if after_eval.starts_with("```") {
                // Markdown风格代码块
                let mut block_lines = vec![];

                if after_eval == "```" {
                    while let Some(next_line) = lines.next() {
                        if next_line.trim() == "```" {
                            break;
                        }
                        block_lines.push(next_line.trim().to_string());
                    }
                } else if let Some(end_pos) = after_eval[3..].find("```") {
                    let inner_code = &after_eval[3..3 + end_pos];
                    block_lines.push(inner_code.trim().to_string());
                }

                let single_line = block_lines.join(" ");
                let escaped = single_line.replace('"', "\\\"");

                // 使用flow项目的表达式解析
                if before_eval.trim().is_empty() {
                    result.push_str(&format!("{}\"{{{{ {} }}}}\"\n", indent, escaped));
                } else {
                    result.push_str(&format!("{}\"{{{{ {} }}}}\"\n", before_eval, escaped));
                }
            } else if after_eval.starts_with("{") {
                // 大括号包裹的代码块
                let mut block_content = String::new();
                let mut brace_count = 0;

                // 处理同一行内容
                for ch in after_eval.chars() {
                    block_content.push(ch);
                    if ch == '{' {
                        brace_count += 1;
                    } else if ch == '}' {
                        brace_count -= 1;
                        if brace_count == 0 {
                            break;
                        }
                    }
                }

                // 处理多行内容
                while brace_count > 0 {
                    if let Some(next_line) = lines.next() {
                        for ch in next_line.chars() {
                            block_content.push(ch);
                            if ch == '{' {
                                brace_count += 1;
                            } else if ch == '}' {
                                brace_count -= 1;
                                if brace_count == 0 {
                                    break;
                                }
                            }
                        }
                    } else {
                        break;
                    }
                }

                // 提取内部内容
                let inner_content =
                    if block_content.starts_with('{') && block_content.ends_with('}') {
                        &block_content[1..block_content.len() - 1]
                    } else {
                        &block_content
                    };

                // 合并为单行
                let single_line = inner_content
                    .lines()
                    .map(|line| line.trim())
                    .filter(|line| !line.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");

                // 转义双引号
                let escaped = single_line.replace('"', "\\\"");

                if before_eval.trim().is_empty() {
                    result.push_str(&format!("{}\"{{{{ {} }}}}\"\n", indent, escaped));
                } else {
                    result.push_str(&format!("{}\"{{{{ {} }}}}\"\n", before_eval, escaped));
                }
            } else if after_eval.starts_with('`') && after_eval.ends_with('`') {
                // 反引号模板字符串
                let inner_content = &after_eval[1..after_eval.len() - 1];
                let escaped = inner_content.replace('"', "\\\"");
                result.push_str(&format!("{}\"{{{{ `{}` }}}}\"\n", before_eval, escaped));
            } else if !after_eval.is_empty() {
                let escaped = after_eval.replace('"', "\\\"");
                result.push_str(&format!("{}\"{{{{ {} }}}}\"\n", before_eval, escaped));
            } else {
                // 缩进代码块
                let mut block_lines = vec![];
                let current_line_indent = line.chars().take_while(|c| c.is_whitespace()).count();

                while let Some(&next_line) = lines.peek() {
                    let line_indent = next_line.chars().take_while(|c| c.is_whitespace()).count();

                    if next_line.trim().is_empty() {
                        lines.next();
                        continue;
                    } else if line_indent > current_line_indent {
                        let content = next_line.trim().to_string();
                        if !content.is_empty() {
                            block_lines.push(content);
                        }
                        lines.next();
                    } else {
                        break;
                    }
                }

                let single_line = block_lines.join(" ");
                let escaped = single_line.replace('"', "\\\"");

                if !escaped.trim().is_empty() {
                    if before_eval.trim().is_empty() {
                        result.push_str(&format!("{}\"{{{{ {} }}}}\"\n", indent, escaped));
                    } else {
                        result.push_str(&format!("{}\"{{{{ {} }}}}\"\n", before_eval, escaped));
                    }
                } else {
                    result.push_str(&format!("{}\n", line));
                }
            }
        } else {
            result.push_str(line);
            result.push('\n');
        }
    }

    result.pop();
    result.to_string()
}

fn parse_include_args(args_str: &str) -> HashMap<String, String> {
    let mut args = HashMap::new();

    if args_str.trim().is_empty() {
        return args;
    }

    // 解析参数格式: key=value key2='value with spaces' key3="quoted value"
    let arg_regex = match Regex::new(r#"(\w+)=(?:'([^']*)'|"([^"]*)"|([^\s]+))"#) {
        Ok(re) => re,
        Err(_) => return args,
    };

    for caps in arg_regex.captures_iter(args_str) {
        let key = caps[1].to_string();
        let value = caps
            .get(2)
            .or(caps.get(3))
            .or(caps.get(4))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        args.insert(key, value);
    }

    args
}

fn process_args_in_content(content: &str, args: &HashMap<String, String>) -> (String, Vec<String>) {
    let mut errors = Vec::new();
    let arg_regex = match Regex::new(r"!arg\s+(\w+)") {
        Ok(re) => re,
        Err(_) => return (content.to_string(), errors),
    };

    let result = arg_regex
        .replace_all(content, |caps: &regex::Captures| {
            let arg_name = &caps[1];
            match args.get(arg_name) {
                Some(value) => value.clone(),
                None => {
                    errors.push(format!("Missing required argument: '{}'", arg_name));
                    format!("<!-- Error: argument '{}' not found -->", arg_name)
                }
            }
        })
        .to_string();

    (result, errors)
}

fn process_include_file(path: &Path, args: &HashMap<String, String>) -> Result<String, String> {
    let path = if path.extension().is_none() {
        let mut new_path = path.to_path_buf();
        new_path.set_extension("phlow");
        new_path
    } else {
        path.to_path_buf()
    };

    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;

    // 处理!arg指令
    let (with_args, arg_errors) = process_args_in_content(&raw, args);

    if !arg_errors.is_empty() {
        return Err(arg_errors.join("; "));
    }

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    // 对包含的文件只处理!include指令
    let (transformed, errors) = preprocessor_directives(&with_args, parent);

    if !errors.is_empty() {
        return Err(errors.join("; "));
    }

    Ok(transformed)
}

fn preprocessor_modules(phlow: &str) -> Result<String, Vec<String>> {
    // 项目专属属性列表
    let exclusive_properties = vec![
        "use",
        "to",
        "id",
        "label",
        "assert",
        "assert_eq",
        "condition",
        "return",
        "payload",
        "input",
        "then",
        "else",
        "steps",
    ];

    // 转义YAML中以!开头的值，避免解析问题
    let escaped_phlow = escape_yaml_exclamation_values(phlow);

    // 解析YAML提取可用模块
    let parsed: Value = match serde_yaml::from_str(&escaped_phlow) {
        Ok(val) => val,
        Err(_) => return Ok(phlow.to_string()),
    };

    let mut available_modules = std::collections::HashSet::new();

    // 从"modules"部分提取模块
    if let Some(modules) = parsed.get("modules") {
        if let Some(modules_array) = modules.as_sequence() {
            for module in modules_array {
                if let Some(module_map) = module.as_mapping() {
                    // 检查是否存在"module"或"name"
                    if let Some(module_name) = module_map
                        .get("module")
                        .or_else(|| module_map.get("name"))
                        .and_then(|v| v.as_str())
                    {
                        // 提取模块名称
                        let clean_name = if module_name.starts_with("./modules/") {
                            &module_name[10..]
                        } else if module_name.contains('/') {
                            module_name.split('/').last().unwrap_or(module_name)
                        } else {
                            module_name
                        };
                        available_modules.insert(clean_name.to_string());
                    }
                }
            }
        }
    }

    if available_modules.is_empty() {
        return Ok(phlow.to_string());
    }

    // 递归转换YAML的函数
    fn transform_value(
        value: &mut Value,
        available_modules: &std::collections::HashSet<String>,
        exclusive_properties: &[&str],
        is_in_transformable_context: bool,
    ) {
        match value {
            Value::Mapping(map) => {
                let mut transformations = Vec::new();

                for (key, val) in map.iter() {
                    if let Some(key_str) = key.as_str() {
                        // 只在可转换上下文中进行转换
                        if is_in_transformable_context {
                            // 如果不是专属属性且是可用模块
                            if !exclusive_properties.contains(&key_str)
                                && available_modules.contains(key_str)
                            {
                                transformations.push((key.clone(), val.clone()));
                            }
                        }
                    }
                }

                // 应用转换
                for (key, old_val) in transformations {
                    map.remove(&key);

                    let mut new_entry = Mapping::new();
                    new_entry.insert(Value::String("use".to_string()), key);
                    new_entry.insert(Value::String("input".to_string()), old_val);

                    // 添加转换后的新条目
                    for (new_key, new_val) in new_entry.iter() {
                        map.insert(new_key.clone(), new_val.clone());
                    }
                }

                // 递归转换
                for (key, val) in map.iter_mut() {
                    let key_str = key.as_str().unwrap_or("");

                    // 确定下一级是否为可转换上下文
                    let next_is_transformable =
                        key_str == "steps" || key_str == "then" || key_str == "else";

                    transform_value(
                        val,
                        available_modules,
                        exclusive_properties,
                        next_is_transformable,
                    );
                }
            }
            Value::Sequence(seq) => {
                for item in seq.iter_mut() {
                    transform_value(
                        item,
                        available_modules,
                        exclusive_properties,
                        is_in_transformable_context,
                    );
                }
            }
            _ => {}
        }
    }

    // 使用转义后的YAML重新解析以进行修改
    let mut parsed_mut: Value = match serde_yaml::from_str(&escaped_phlow) {
        Ok(val) => val,
        Err(_) => return Ok(phlow.to_string()),
    };

    transform_value(
        &mut parsed_mut,
        &available_modules,
        &exclusive_properties,
        false,
    );

    // 转换回YAML并取消转义
    match serde_yaml::to_string(&parsed_mut) {
        Ok(result) => Ok(unescape_yaml_exclamation_values(&result)),
        Err(_) => Ok(phlow.to_string()),
    }
}

// 转义以!开头的值，避免被解析为YAML标签
fn escape_yaml_exclamation_values(yaml: &str) -> String {
    let regex = match Regex::new(r"((?::\s*|-\s+\w+:\s*))(!\w.*?)\s*$") {
        Ok(re) => re,
        Err(_) => return yaml.to_string(),
    };

    let result = regex
        .replace_all(yaml, |caps: &regex::Captures| {
            let prefix = &caps[1];
            let exclamation_value = &caps[2];
            format!(r#"{} "__RUNTIME_ESCAPE__{}""#, prefix, exclamation_value)
        })
        .to_string();

    result
}

// 取消转义带有!的值
fn unescape_yaml_exclamation_values(yaml: &str) -> String {
    let regex = match Regex::new(r"__RUNTIME_ESCAPE__(!\w[^\s]*)") {
        Ok(re) => re,
        Err(_) => return yaml.to_string(),
    };

    let result = regex
        .replace_all(yaml, |caps: &regex::Captures| {
            let exclamation_value = &caps[1];
            exclamation_value.to_string()
        })
        .to_string();

    result
}
    