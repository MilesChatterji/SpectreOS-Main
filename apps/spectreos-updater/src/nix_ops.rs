#[derive(Clone, Debug)]
pub struct Package {
    pub pname: String,
    pub version: String,
    pub description: String,
    pub is_unstable: bool,
}

// Public read-only credentials used by search.nixos.org (proxied via Netlify).
// If either value changes: browser dev tools on search.nixos.org → Network tab →
// search for anything → inspect the POST request's Authorization header and URL path.
const ES_SERVER: &str = "https://search.nixos.org/backend";
const ES_AUTH_B64: &str = "YVdWU0FMWHBadjpYOGdQSG56TDUyd0ZFZWt1eHNmUTljU2g=";
// Generation prefix increments when NixOS reindexes. Check the URL in dev tools if results stop.
// Confirmed same generation index works for both stable and unstable.
const ES_GENERATION: u32 = 47;
// SpectreOS is based on NixOS 25.11. Update when the base NixOS version changes.
const NIXOS_VERSION: &str = "25.11";
const NIXOS_UNSTABLE_VERSION: &str = "unstable";

const UPDATER_MARKER: &str =
    "# SpectreOS Updater managed packages — do not edit this block manually";
const UPDATER_END_MARKER: &str = "# END SpectreOS Updater";

fn home_nix_path() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    format!("{}/.config/home-manager/home.nix", home)
}

// HTTP call to the NixOS Elasticsearch backend — no local Nix evaluation, instant results.
// Requires internet, which is consistent with needing internet to actually install packages.
// When include_unstable is true, searches both channels and merges; unstable wins on pname clash.
pub fn search(query: &str, include_unstable: bool) -> Vec<Package> {
    let mut stable = search_channel(query, NIXOS_VERSION, false);

    if !include_unstable {
        return stable;
    }

    let unstable = search_channel(query, NIXOS_UNSTABLE_VERSION, true);

    // Unstable results take precedence — build a set of pnames already covered by unstable.
    let unstable_pnames: std::collections::HashSet<&str> =
        unstable.iter().map(|p| p.pname.as_str()).collect();
    stable.retain(|p| !unstable_pnames.contains(p.pname.as_str()));

    // Unstable first so the newer versions appear at the top.
    let mut merged = unstable;
    merged.extend(stable);
    merged
}

fn search_channel(query: &str, version: &str, is_unstable: bool) -> Vec<Package> {
    let url = format!("{}/latest-{}-nixos-{}/_search", ES_SERVER, ES_GENERATION, version);
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
                    Some(Package { pname, version, description, is_unstable })
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

/// Packages in the updater-managed block of home.nix (can be removed via the updater).
pub fn read_installed_packages() -> Vec<String> {
    let content = std::fs::read_to_string(home_nix_path()).unwrap_or_default();
    let mut in_block = false;
    let mut in_list = false;
    let mut packages = Vec::new();

    for line in content.lines() {
        let t = line.trim();
        if t.starts_with(UPDATER_MARKER) {
            in_block = true;
            continue;
        }
        if t.starts_with(UPDATER_END_MARKER) {
            break;
        }
        if in_block && !in_list && t.starts_with("home.packages") && t.contains('[') {
            in_list = true;
            continue;
        }
        if in_block && in_list {
            if t.starts_with(']') { in_list = false; continue; }
            if !t.is_empty() && !t.starts_with('#') {
                packages.push(t.to_string());
            }
        }
    }
    packages
}

/// All packages from every home.packages block in home.nix (for search ✓ indicators).
/// Uses the last component of dotted names so e.g. unstable.spotify → spotify.
pub fn read_all_home_packages() -> Vec<String> {
    let content = std::fs::read_to_string(home_nix_path()).unwrap_or_default();
    let mut in_list = false;
    let mut packages = Vec::new();

    for line in content.lines() {
        let t = line.trim();
        if !in_list && t.starts_with("home.packages") && t.contains("with pkgs") && t.contains('[') {
            in_list = true;
            continue;
        }
        if in_list {
            if t.starts_with(']') { in_list = false; continue; }
            if t.is_empty() || t.starts_with('#') { continue; }
            let token = t.split_whitespace().next().unwrap_or("");
            if !token.is_empty() {
                let pname = token.split('.').last().unwrap_or(token);
                packages.push(pname.to_string());
            }
        }
    }
    packages
}

/// Write the updater block into home.nix, replacing any existing block or appending before `}`.
/// Backs up home.nix to home.nix.updater-bak first.
pub fn write_extra_packages(packages: &[String]) -> std::io::Result<()> {
    let path = home_nix_path();
    let content = std::fs::read_to_string(&path)?;

    let _ = std::fs::copy(&path, format!("{}.updater-bak", path));

    let pkg_lines: String = packages
        .iter()
        .map(|p| format!("    {}", p))
        .collect::<Vec<_>>()
        .join("\n");

    let new_block = if packages.is_empty() {
        format!("  {}\n  {}", UPDATER_MARKER, UPDATER_END_MARKER)
    } else {
        format!(
            "  {}\n  home.packages = with pkgs; [\n{}\n  ];\n  {}",
            UPDATER_MARKER, pkg_lines, UPDATER_END_MARKER
        )
    };

    let new_content = if content.lines().any(|l| l.trim().starts_with(UPDATER_MARKER)) {
        replace_updater_block(&content, &new_block)
    } else {
        insert_before_last_brace(&content, &new_block)
    };

    std::fs::write(&path, new_content)
}

fn replace_updater_block(content: &str, new_block: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.iter().position(|l| l.trim().starts_with(UPDATER_MARKER));
    let end = lines.iter().position(|l| l.trim().starts_with(UPDATER_END_MARKER));

    match (start, end) {
        (Some(s), Some(e)) => {
            let mut result: Vec<&str> = lines[..s].to_vec();
            result.extend(new_block.lines());
            result.extend_from_slice(&lines[e + 1..]);
            let joined = result.join("\n");
            if content.ends_with('\n') { joined + "\n" } else { joined }
        }
        _ => format!("{}\n{}\n", content.trim_end_matches('\n'), new_block),
    }
}

fn insert_before_last_brace(content: &str, new_block: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    match lines.iter().rposition(|l| l.trim() == "}") {
        Some(idx) => {
            let mut result: Vec<&str> = lines[..idx].to_vec();
            result.push("");
            result.extend(new_block.lines());
            result.push(lines[idx]);
            result.join("\n") + "\n"
        }
        None => format!("{}\n{}\n", content.trim_end_matches('\n'), new_block),
    }
}

pub fn run_home_manager() -> Result<(), String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let existing_path = std::env::var("PATH").unwrap_or_default();
    // Prepend common nix profile locations in case the GUI was launched without a full login shell.
    let extended_path = format!(
        "{}/.nix-profile/bin:/run/current-system/sw/bin:/nix/var/nix/profiles/default/bin:{}",
        home, existing_path
    );

    let output = std::process::Command::new("home-manager")
        .env("PATH", &extended_path)
        .args(["switch", "-b", "backup", "--option", "max-jobs", "2", "--option", "cores", "2"])
        .output()
        .map_err(|e| format!("failed to launch home-manager: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail: String = stderr
            .lines()
            .rev()
            .take(6)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        Err(format!(
            "home-manager exited with code {}{}",
            output.status.code().unwrap_or(-1),
            if !tail.is_empty() { format!("\n{}", tail) } else { String::new() }
        ))
    }
}
