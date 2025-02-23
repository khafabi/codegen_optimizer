use regex::Regex;
use serde_yaml::{Value};
use std::collections::HashMap;
use std::error::Error;
use std::fs::{read_to_string, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use log::{info, warn, error};
use simple_logger::SimpleLogger;
use std::process::Command;

fn run_command(cmd: &str, args: &[&str]) -> Result<(), Box<dyn Error>> {
    let full_command = format!("{} {}", cmd, args.join(" "));
    info!("Executing: {}", full_command);
    
    let start = std::time::Instant::now();
    let output = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                format!("Command '{}' not found. Please ensure it is installed and in your PATH", cmd)
            } else {
                format!("Failed to execute command '{}': {}", cmd, e)
            }
        })?;

    let duration = start.elapsed();
    info!("Command completed in {:.2} seconds", duration.as_secs_f64());

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Command '{} {}' failed with status {}: {}",
            cmd,
            args.join(" "),
            output.status,
            stderr
        ).into());
    }
    Ok(())
}

fn check_flutter_installed() -> Result<(), Box<dyn Error>> {
    // Get the PATH environment variable
    let path_var = std::env::var("PATH").unwrap_or_else(|_| "PATH not set".to_string());
    info!("Current PATH: {}", path_var);

    // Try to find flutter in PATH
    let flutter_path = which::which("flutter")
        .map_err(|e| format!("Failed to find flutter in PATH: {}\nPATH: {}", e, path_var))?;
    info!("Found flutter at: {}", flutter_path.display());

    // On Windows, we need to use flutter.bat
    let flutter_cmd = if cfg!(windows) {
        "flutter.bat"
    } else {
        "flutter"
    };
    
    // Try running flutter --version
    run_command(flutter_cmd, &["--version"])?;
    Ok(())
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
enum AnnotationType {
    CopyWith,
    JsonSerializable,
    Hive,
}

#[derive(Debug, Clone)]
struct AnnotationPattern {
    pattern: &'static str,
    builder_key: &'static str,
}

impl AnnotationPattern {
    fn compile(&self) -> Regex {
        Regex::new(self.pattern).unwrap()
    }
}

struct PatternRegistry;

impl PatternRegistry {
    fn get_patterns() -> HashMap<AnnotationType, AnnotationPattern> {
        let mut map = HashMap::new();
        map.insert(
            AnnotationType::CopyWith,
            AnnotationPattern {
                pattern: r"@CopyWith\s*\(",
                builder_key: "copy_with_extension_gen",
            },
        );
        map.insert(
            AnnotationType::JsonSerializable,
            AnnotationPattern {
                pattern: r"@JsonSerializable\s*\(",
                builder_key: "json_serializable",
            },
        );
        map.insert(
            AnnotationType::Hive,
            AnnotationPattern {
                pattern: r"@HiveType\s*\(",
                builder_key: "hive_generator",
            },
        );
        map
    }

    fn get_pattern(annotation_type: &AnnotationType) -> Option<AnnotationPattern> {
        Self::get_patterns().get(annotation_type).cloned()
    }
}

struct BuildYamlGenerator {
    working_dir: PathBuf,
    build_yaml_path: PathBuf,
}

impl BuildYamlGenerator {
    fn new(working_dir: PathBuf) -> Self {
        let build_yaml_path = working_dir.join("build.yaml");
        Self {
            working_dir,
            build_yaml_path,
        }
    }

    fn read_yaml_file(&self) -> Result<Value, Box<dyn Error>> {
        let content = read_to_string(&self.build_yaml_path)?;
        let yaml: Value = serde_yaml::from_str(&content)?;
        Ok(yaml)
    }

    fn find_files_with_annotation(&self, annotation_type: &AnnotationType) -> Result<Vec<String>, Box<dyn Error>> {
        let pattern_info = PatternRegistry::get_pattern(annotation_type)
            .ok_or_else(|| format!("Unsupported annotation type: {:?}", annotation_type))?;
        let regex = pattern_info.compile();

        let mut files_with_annotation = Vec::new();

        for entry in WalkDir::new(&self.working_dir).into_iter().filter_map(|e| e.ok()) {
            if entry.path().extension().and_then(|s| s.to_str()) == Some("dart") {
                match std::fs::read_to_string(entry.path()) {
                    Ok(content) => {
                        if regex.is_match(&content) {
                            let processed = self.process_part_of(entry.path(), &content);
                            files_with_annotation.push(processed.display().to_string());
                        }
                    }
                    Err(e) => {
                        warn!("Error processing file {:?}: {}", entry.path(), e);
                        continue;
                    }
                }
            }
        }

        // Wrap with quotes as in Python code.
        let mut quoted_files: Vec<String> = files_with_annotation
            .iter()
            .map(|f| format!("\"{}\"", f))
            .collect();
        quoted_files.sort();
        Ok(quoted_files)
    }

    fn process_part_of(&self, file_path: &Path, content: &str) -> PathBuf {
        if content.contains("part of") {
            if let Some(idx) = content.find("part of ") {
                let after = &content[idx + "part of ".len()..];
                if let Some(end_idx) = after.find(';') {
                    let parent_file = after[..end_idx].trim();
                    return file_path.parent().unwrap_or_else(|| Path::new("")).join(parent_file);
                }
            }
        }
        file_path.to_path_buf()
    }

    fn format_build_yaml(&self) -> Result<(), Box<dyn Error>> {
        let content = read_to_string(&self.build_yaml_path)?;
        let formatted_content = content
            .replace('\'', "")
            .replace(
                &format!("{}{}", self.working_dir.display(), std::path::MAIN_SEPARATOR),
                "",
            )
            .replace(std::path::MAIN_SEPARATOR, "/");
        let mut file = File::create(&self.build_yaml_path)?;
        file.write_all(formatted_content.as_bytes())?;
        Ok(())
    }

    fn update_build_yaml(&self) -> Result<(), Box<dyn Error>> {
        info!("Generating build.yaml for {:?}", &self.working_dir);
        let mut yaml_content = self.read_yaml_file()?;

        let patterns = PatternRegistry::get_patterns();
        for (annotation_type, pattern_info) in patterns.iter() {
            if let Ok(files) = self.find_files_with_annotation(annotation_type) {
                // Navigate the YAML structure to update the generate_for field
                // Assuming the YAML structure matches the Python code.
                if let Some(targets) = yaml_content.get_mut("targets") {
                    if let Some(default) = targets.get_mut("$default") {
                        if let Some(builders) = default.get_mut("builders") {
                            if let Some(builder) = builders.get_mut(pattern_info.builder_key) {
                                if let Some(generate_for) = builder.get_mut("generate_for") {
                                    // Replace with new list of files.
                                    *generate_for = Value::Sequence(files.into_iter().map(Value::String).collect());
                                }
                            }
                        }
                    }
                }
            }
        }

        // Write YAML back
        {
            let file = File::create(&self.build_yaml_path)?;
            serde_yaml::to_writer(file, &yaml_content)?;
        }

        self.format_build_yaml()?;
        info!("Successfully updated build.yaml");
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    SimpleLogger::new().init().unwrap();

    // Check if flutter is installed before proceeding
    if let Err(e) = check_flutter_installed() {
        error!("{}", e);
        error!("Please install Flutter and ensure it's in your PATH before running this tool.");
        error!("You can download Flutter from: https://flutter.dev");
        return Err(e);
    }

    let current_dir = std::env::current_dir()?;
    let generator = BuildYamlGenerator::new(current_dir);
    match generator.update_build_yaml() {
        Ok(_) => {
            // Run Flutter commands sequentially after update_build_yaml
            let flutter_cmd = if cfg!(windows) {
                "flutter.bat"
            } else {
                "flutter"
            };
            
            run_command(flutter_cmd, &["clean"])?;
            run_command(flutter_cmd, &["pub", "upgrade"])?;
            run_command(flutter_cmd, &["pub", "get"])?;
            run_command(flutter_cmd, &["pub", "run", "build_runner", "build", "--delete-conflicting-outputs"])?;
            Ok(())
        },
        Err(e) => {
            error!("Failed to generate build.yaml: {}", e);
            Err(e)
        }
    }
}
