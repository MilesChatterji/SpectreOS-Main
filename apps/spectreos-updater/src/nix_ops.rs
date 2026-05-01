#[derive(Clone, Debug)]
pub struct Package {
    pub pname: String,
    pub version: String,
    pub description: String,
}

// Public read-only credentials used by search.nixos.org (proxied via Netlify).
// If either value changes: browser dev tools on search.nixos.org → Network tab →
// search for anything → inspect the POST request's Authorization header and URL path.
const ES_SERVER: &str = "https://search.nixos.org/backend";
const ES_AUTH_B64: &str = "YVdWU0FMWHBadjpYOGdQSG56TDUyd0ZFZWt1eHNmUTljU2g=";
// Generation prefix increments when NixOS reindexes. Check the URL in dev tools if results stop.
const ES_GENERATION: u32 = 47;
// SpectreOS is based on NixOS 25.11. Update when the base NixOS version changes.
const NIXOS_VERSION: &str = "25.11";

// HTTP call to the NixOS Elasticsearch backend — no local Nix evaluation, instant results.
// Requires internet, which is consistent with needing internet to actually install packages.
pub fn search(query: &str) -> Vec<Package> {
    let url = format!("{}/latest-{}-nixos-{}/_search", ES_SERVER, ES_GENERATION, NIXOS_VERSION);
    let q = json_escape(query);
    let body = format!(
        r#"{{"from":0,"size":50,"query":{{"bool":{{"should":[{{"match":{{"package_pname":{{"query":"{q}","boost":3}}}}}},{{"match":{{"package_description":{{"query":"{q}","boost":1}}}}}},{{"match":{{"package_attr_name":{{"query":"{q}"}}}}}}],"minimum_should_match":1}}}}}}"#,
        q = q
    );

    let auth_header = format!("Authorization: Basic {}", ES_AUTH_B64);
    let output = std::process::Command::new("curl")
        .args([
            "-s",
            "--max-time", "10",
            "-H", &auth_header,
            "-H", "Content-Type: application/json",
            "-d", &body,
            &url,
        ])
        .output();

    let bytes = match output {
        Ok(o) if !o.stdout.is_empty() => o.stdout,
        _ => return Vec::new(),
    };

    let json: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(j) => j,
        Err(_) => return Vec::new(),
    };

    json["hits"]["hits"]
        .as_array()
        .map(|hits| {
            hits.iter()
                .filter_map(|hit| {
                    let src = &hit["_source"];
                    let pname = src["package_pname"].as_str()?.to_string();
                    let version = src["package_pversion"].as_str().unwrap_or("").to_string();
                    let description = src["package_description"].as_str().unwrap_or("").to_string();
                    Some(Package { pname, version, description })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn json_escape(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            '"' => vec!['\\', '"'],
            '\\' => vec!['\\', '\\'],
            '\n' => vec!['\\', 'n'],
            '\r' => vec!['\\', 'r'],
            '\t' => vec!['\\', 't'],
            c => vec![c],
        })
        .collect()
}

pub fn read_installed_packages() -> Vec<String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let path = format!("{}/.config/spectreos/extra-packages.nix", home);
    let content = std::fs::read_to_string(path).unwrap_or_default();

    let mut in_list = false;
    let mut packages = Vec::new();
    for line in content.lines() {
        let t = line.trim();
        if t.contains("home.packages = with pkgs; [") {
            in_list = true;
            continue;
        }
        if in_list {
            if t.starts_with(']') { break; }
            if !t.is_empty() {
                packages.push(t.to_string());
            }
        }
    }
    packages
}

pub fn write_extra_packages(packages: &[String]) -> std::io::Result<()> {
    let home = std::env::var("HOME").map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::NotFound, e.to_string())
    })?;
    let config_dir = format!("{}/.config/spectreos", home);
    std::fs::create_dir_all(&config_dir)?;

    let pkg_lines: String = packages
        .iter()
        .map(|p| format!("    {}", p))
        .collect::<Vec<_>>()
        .join("\n");

    let content = format!(
        "# Managed by SpectreOS Updater — do not edit manually\n\
         {{ pkgs, ... }}: {{\n\
           home.packages = with pkgs; [\n\
         {}\n\
           ];\n\
         }}\n",
        pkg_lines
    );

    std::fs::write(format!("{}/extra-packages.nix", config_dir), content)
}

pub fn run_home_manager() -> Result<(), String> {
    let status = std::process::Command::new("home-manager")
        .args([
            "switch",
            "-b", "backup",
            "--option", "max-jobs", "2",
            "--option", "cores", "2",
        ])
        .status()
        .map_err(|e| e.to_string())?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "home-manager exited with code {}",
            status.code().unwrap_or(-1)
        ))
    }
}
