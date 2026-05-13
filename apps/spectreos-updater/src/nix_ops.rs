use std::collections::HashMap;

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
// Confirmed generation 47 serves both the stable and unstable indices.
const ES_GENERATION: u32 = 47;
// SpectreOS is based on NixOS 25.11. Update when the base NixOS version changes.
const NIXOS_VERSION: &str = "25.11";
const NIXOS_UNSTABLE_VERSION: &str = "unstable";

// Inline markers — live INSIDE the existing home.packages = with pkgs; [...] block.
// Using a second home.packages = ... definition in the same attrset is a Nix error, so
// we add package names only (no attribute wrapper) and bracket them with comments.
const UPDATER_INLINE_START: &str =
    "# SpectreOS Updater managed packages — do not edit below";
const UPDATER_END: &str = "# END SpectreOS Updater";

// Legacy marker — used in the old separate-block format that wrote its own home.packages = ...
// Detected so we can migrate to the inline format on the next apply.
const UPDATER_LEGACY_START: &str =
    "# SpectreOS Updater managed packages — do not edit this block manually";

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

    // Unstable results take precedence — filter out stable entries with the same pname.
    let unstable_pnames: std::collections::HashSet<&str> =
        unstable.iter().map(|p| p.pname.as_str()).collect();
    stable.retain(|p| !unstable_pnames.contains(p.pname.as_str()));

    // Unstable first so newer versions surface at the top.
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


/// Read the installed versions stored in the `# @versions` comment inside the managed block.
pub fn read_installed_versions() -> HashMap<String, String> {
    let content = std::fs::read_to_string(home_nix_path()).unwrap_or_default();
    parse_versions_comment(&content)
}

fn parse_versions_comment(content: &str) -> HashMap<String, String> {
    for line in content.lines() {
        if let Some(rest) = line.trim().strip_prefix("# @versions ") {
            return rest
                .split_whitespace()
                .filter_map(|kv| {
                    let mut it = kv.splitn(2, '=');
                    Some((it.next()?.to_string(), it.next()?.to_string()))
                })
                .collect();
        }
    }
    HashMap::new()
}

/// Fetch the current available version for each managed package from the nixpkgs ES index.
/// `managed` is a slice of (pname, is_unstable) pairs. Results are keyed by pname.
pub fn fetch_available_versions(managed: &[(String, bool)]) -> HashMap<String, String> {
    let mut result = HashMap::new();
    for (pname, is_unstable) in managed {
        let channel = if *is_unstable { NIXOS_UNSTABLE_VERSION } else { NIXOS_VERSION };
        let hits = search_channel(pname, channel, *is_unstable);
        if let Some(pkg) = hits.into_iter().find(|p| &p.pname == pname) {
            result.insert(pname.clone(), pkg.version);
        }
    }
    result
}

/// Packages managed by the updater (inline section inside home.packages in home.nix).
/// Falls back to the legacy separate-block format for migration reads.
pub fn read_installed_packages() -> Vec<String> {
    let content = std::fs::read_to_string(home_nix_path()).unwrap_or_default();
    if content.lines().any(|l| l.trim().starts_with(UPDATER_INLINE_START)) {
        read_inline_packages(&content)
    } else {
        read_legacy_packages(&content)
    }
}

fn read_inline_packages(content: &str) -> Vec<String> {
    let mut in_section = false;
    let mut packages = Vec::new();
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with(UPDATER_INLINE_START) { in_section = true; continue; }
        if t.starts_with(UPDATER_END) { break; }
        if in_section && !t.is_empty() && !t.starts_with('#') {
            packages.push(t.to_string());
        }
    }
    packages
}

fn read_legacy_packages(content: &str) -> Vec<String> {
    let mut in_block = false;
    let mut in_list = false;
    let mut packages = Vec::new();
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with(UPDATER_LEGACY_START) { in_block = true; continue; }
        if in_block && t.starts_with(UPDATER_END) { break; }
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


/// Write the updater's packages as an inline section inside the existing home.packages block.
/// Also writes a `# @versions` comment to track installed versions for update detection.
///
/// If the legacy separate-block format is detected it is removed first (migration), then the
/// packages are written inline. This avoids the "attribute 'home' already defined" Nix error
/// caused by having two `home.packages = ...` definitions in the same attrset.
pub fn write_extra_packages(packages: &[String], versions: &HashMap<String, String>) -> std::io::Result<()> {
    let path = home_nix_path();
    let content = std::fs::read_to_string(&path)?;
    let _ = std::fs::copy(&path, format!("{}.updater-bak", path));

    let versions_line = if versions.is_empty() {
        String::new()
    } else {
        let mut pairs: Vec<String> = versions.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
        pairs.sort();
        format!("\n    # @versions {}", pairs.join(" "))
    };

    let pkg_lines: String = packages
        .iter()
        .map(|p| format!("    {}", p))
        .collect::<Vec<_>>()
        .join("\n");

    let inline_section = if packages.is_empty() {
        format!("    {}{}\n    {}", UPDATER_INLINE_START, versions_line, UPDATER_END)
    } else {
        format!("    {}{}\n{}\n    {}", UPDATER_INLINE_START, versions_line, pkg_lines, UPDATER_END)
    };

    // Step 1: migrate away from legacy separate block if present.
    let has_legacy = content.lines().any(|l| l.trim().starts_with(UPDATER_LEGACY_START));
    let content = if has_legacy {
        remove_legacy_block(&content)
    } else {
        content
    };

    // Step 2: insert or replace the inline section.
    let new_content = if content.lines().any(|l| l.trim().starts_with(UPDATER_INLINE_START)) {
        replace_inline_section(&content, &inline_section)
    } else {
        insert_inline_in_home_packages(&content, &inline_section)
    };

    std::fs::write(&path, new_content)
}

/// Remove the legacy standalone updater block (the one that had its own `home.packages = ...`).
fn remove_legacy_block(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let start = match lines.iter().position(|l| l.trim().starts_with(UPDATER_LEGACY_START)) {
        Some(i) => i,
        None => return content.to_string(),
    };
    let end = match lines.iter().skip(start).position(|l| l.trim().starts_with(UPDATER_END)) {
        Some(i) => start + i,
        None => return content.to_string(),
    };
    // Also consume a leading blank line before the block.
    let trim_start = if start > 0 && lines[start - 1].trim().is_empty() {
        start - 1
    } else {
        start
    };
    let mut result: Vec<&str> = lines[..trim_start].to_vec();
    result.extend_from_slice(&lines[end + 1..]);
    let joined = result.join("\n");
    if content.ends_with('\n') { joined + "\n" } else { joined }
}

/// Replace an existing inline section (start..=end markers) with new content.
fn replace_inline_section(content: &str, inline_section: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let start = match lines.iter().position(|l| l.trim().starts_with(UPDATER_INLINE_START)) {
        Some(i) => i,
        None => return content.to_string(),
    };
    let end = match lines.iter().skip(start).position(|l| l.trim().starts_with(UPDATER_END)) {
        Some(i) => start + i,
        None => return content.to_string(),
    };
    let mut result: Vec<&str> = lines[..start].to_vec();
    result.extend(inline_section.lines());
    result.extend_from_slice(&lines[end + 1..]);
    let joined = result.join("\n");
    if content.ends_with('\n') { joined + "\n" } else { joined }
}

/// Insert the inline section before the closing `];` of the first home.packages block.
/// If no home.packages block exists, creates one (handles the VM hm-entry.nix case).
fn insert_inline_in_home_packages(content: &str, inline_section: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();

    let pkg_start = lines.iter().position(|l| {
        let t = l.trim();
        t.starts_with("home.packages") && t.contains("with pkgs") && t.contains('[')
    });

    match pkg_start {
        None => {
            // No home.packages block — create one containing just the updater section.
            let new_block = format!(
                "  home.packages = with pkgs; [\n{}\n  ];",
                inline_section
            );
            insert_before_last_brace(content, &new_block)
        }
        Some(start_idx) => {
            // Locate the closing `];` by tracking bracket depth.
            let mut depth = 1i32;
            let mut close_idx = None;
            for (i, line) in lines.iter().enumerate().skip(start_idx + 1) {
                let t = line.trim();
                depth += t.chars().filter(|&c| c == '[').count() as i32;
                depth -= t.chars().filter(|&c| c == ']').count() as i32;
                if depth <= 0 {
                    close_idx = Some(i);
                    break;
                }
            }
            match close_idx {
                None => content.to_string(),
                Some(end_idx) => {
                    let mut result: Vec<&str> = lines[..end_idx].to_vec();
                    result.push("");
                    result.extend(inline_section.lines());
                    result.extend_from_slice(&lines[end_idx..]);
                    let joined = result.join("\n");
                    if content.ends_with('\n') { joined + "\n" } else { joined }
                }
            }
        }
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

pub fn nixos_version() -> String {
    std::fs::read_to_string("/run/current-system/nixos-version")
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// Compute the next NixOS release from a version string like "25.11.20260501.abc1234".
/// NixOS releases: YY.05 (May) and YY.11 (November).
pub fn next_nixos_version(current: &str) -> Option<String> {
    let parts: Vec<&str> = current.split('.').collect();
    if parts.len() < 2 { return None; }
    let year: u32 = parts[0].trim().parse().ok()?;
    let month_str: String = parts[1].chars().take_while(|c| c.is_ascii_digit()).collect();
    let month: u32 = month_str.parse().ok()?;
    let (ny, nm) = if month < 11 { (year, 11u32) } else { (year + 1, 5u32) };
    Some(format!("{}.{:02}", ny, nm))
}

/// Returns true if the channels.nixos.org channel for `version` responds with a redirect/200.
pub fn check_upgrade_available(version: &str) -> bool {
    let url = format!("https://channels.nixos.org/nixos-{}", version);
    let output = std::process::Command::new("curl")
        .args([
            "-s", "-o", "/dev/null",
            "-w", "%{http_code}",
            "--max-time", "10",
            "--head",
            &url,
        ])
        .output();
    match output {
        Ok(o) => {
            let c = String::from_utf8_lossy(&o.stdout);
            let c = c.trim();
            c == "200" || c == "301" || c == "302"
        }
        Err(_) => false,
    }
}

/// Run the system upgrade via the sudoers-allowed helper script at /etc/spectreos/upgrade-helper.sh.
pub fn run_system_upgrade(version: &str) -> Result<(), String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let existing_path = std::env::var("PATH").unwrap_or_default();
    let extended_path = format!(
        "{}/.nix-profile/bin:/run/current-system/sw/bin:/nix/var/nix/profiles/default/bin:{}",
        home, existing_path
    );
    let cmd = format!("/run/wrappers/bin/sudo /etc/spectreos/upgrade-helper.sh {}", shell_escape(version));
    let output = std::process::Command::new("bash")
        .env("PATH", &extended_path)
        .env("HOME", &home)
        .args(["-l", "-c", &cmd])
        .output()
        .map_err(|e| format!("failed to launch upgrade: {}", e))?;

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
            "upgrade exited with code {}{}",
            output.status.code().unwrap_or(-1),
            if !tail.is_empty() { format!("\n{}", tail) } else { String::new() }
        ))
    }
}

pub fn run_system_rebuild() -> Result<(), String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let existing_path = std::env::var("PATH").unwrap_or_default();
    let extended_path = format!(
        "{}/.nix-profile/bin:/run/current-system/sw/bin:/nix/var/nix/profiles/default/bin:{}",
        home, existing_path
    );
    let output = std::process::Command::new("bash")
        .env("PATH", &extended_path)
        .env("HOME", &home)
        .args(["-l", "-c", "/run/wrappers/bin/sudo /etc/spectreos/rebuild-helper.sh"])
        .output()
        .map_err(|e| format!("failed to launch rebuild: {}", e))?;

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
            "rebuild exited with code {}{}",
            output.status.code().unwrap_or(-1),
            if !tail.is_empty() { format!("\n{}", tail) } else { String::new() }
        ))
    }
}

#[derive(Debug, Clone)]
pub struct NixosGeneration {
    pub id: u32,
    pub date: String,
    pub current: bool,
}

pub fn list_generations() -> Result<Vec<NixosGeneration>, String> {
    let output = std::process::Command::new("/run/wrappers/bin/sudo")
        .args(["/etc/spectreos/list-generations-helper.sh"])
        .output()
        .map_err(|e| format!("failed to list generations: {}", e))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut gens: Vec<NixosGeneration> = stdout
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 3 { return None; }
            let id: u32 = parts[0].parse().ok()?;
            let date = format!("{} {}", parts[1], parts[2]);
            let current = line.contains("(current)");
            Some(NixosGeneration { id, date, current })
        })
        .collect();
    gens.sort_by(|a, b| b.id.cmp(&a.id));
    Ok(gens)
}

pub fn run_system_rollback(generation: u32) -> Result<(), String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let existing_path = std::env::var("PATH").unwrap_or_default();
    let extended_path = format!(
        "{}/.nix-profile/bin:/run/current-system/sw/bin:/nix/var/nix/profiles/default/bin:{}",
        home, existing_path
    );
    let cmd = format!("/run/wrappers/bin/sudo /etc/spectreos/rollback-helper.sh {}", generation);
    let output = std::process::Command::new("bash")
        .env("PATH", &extended_path)
        .env("HOME", &home)
        .args(["-l", "-c", &cmd])
        .output()
        .map_err(|e| format!("failed to launch rollback: {}", e))?;

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
            "rollback exited with code {}{}",
            output.status.code().unwrap_or(-1),
            if !tail.is_empty() { format!("\n{}", tail) } else { String::new() }
        ))
    }
}

fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Read the nix-path / extra-nix-path entries from /etc/nix/nix.conf.
/// Returns a colon-separated string suitable for appending to NIX_PATH.
/// nix.nixPath in configuration.nix writes here, but bash -l only picks up
/// per-user channel paths. If NIX_PATH is set, Nix ignores nix.conf's nix-path,
/// so we must append it explicitly to NIX_PATH before running home-manager.
fn nix_conf_search_path() -> String {
    let conf = std::fs::read_to_string("/etc/nix/nix.conf").unwrap_or_default();
    let mut paths: Vec<String> = Vec::new();
    for line in conf.lines() {
        let t = line.trim();
        let val = if let Some(v) = t.strip_prefix("nix-path=").or_else(|| t.strip_prefix("nix-path =")) {
            v.trim()
        } else if let Some(v) = t.strip_prefix("extra-nix-path=").or_else(|| t.strip_prefix("extra-nix-path =")) {
            v.trim()
        } else {
            continue;
        };
        for entry in val.split_whitespace() {
            if !entry.is_empty() {
                paths.push(entry.to_string());
            }
        }
    }
    paths.join(":")
}

pub fn run_home_manager() -> Result<(), String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let existing_path = std::env::var("PATH").unwrap_or_default();
    let extended_path = format!(
        "{}/.nix-profile/bin:/run/current-system/sw/bin:/nix/var/nix/profiles/default/bin:{}",
        home, existing_path
    );

    // bash -l sources /etc/profile which sets NIX_PATH from per-user channels.
    // nix.nixPath in configuration.nix writes home-manager into nix.conf's nix-path,
    // but Nix ignores nix.conf when NIX_PATH is already set in the environment.
    // We append the nix.conf entries to whatever bash -l sets so both are visible.
    let conf_path = nix_conf_search_path();
    let cmd = if conf_path.is_empty() {
        "home-manager switch -b backup --option max-jobs 2 --option cores 2".to_string()
    } else {
        format!(
            "export NIX_PATH=\"${{NIX_PATH:+$NIX_PATH:}}{}\"; home-manager switch -b backup --option max-jobs 2 --option cores 2",
            conf_path
        )
    };

    let output = std::process::Command::new("bash")
        .env("PATH", &extended_path)
        .env("HOME", &home)
        .env_remove("NIX_PATH")
        .args(["-l", "-c", &cmd])
        .output()
        .map_err(|e| format!("failed to launch bash: {}", e))?;

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
