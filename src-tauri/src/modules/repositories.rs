//! APT Repository Manager module - Enhanced
//! Manages sources.list and PPAs with region detection and apt-fast support

use crate::error::{AppError, Result};
use crate::utils::privileged;
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::Instant;
use tokio::time::{timeout, Duration};

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub file_path: String,
    pub line_number: usize,
    pub repo_type: String,      // deb or deb-src
    pub uri: String,
    pub suite: String,          // noble, noble-updates, etc.
    pub components: Vec<String>, // main, restricted, universe, multiverse
    pub is_enabled: bool,
    pub is_ppa: bool,
    pub raw_line: String,
    pub ppa_name: Option<String>, // For PPA: "ppa:user/repo"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorInfo {
    pub name: String,
    pub uri: String,
    pub country: String,
    pub country_code: String,
    pub latency_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionInfo {
    pub detected_country: String,
    pub detected_code: String,
    pub available_regions: Vec<(String, String)>, // (code, name)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AptFastStatus {
    pub installed: bool,
    pub aria2_installed: bool,
    pub max_connections: u32,
}

// ============================================================================
// Ubuntu/Debian Mirrors - Global (100+ mirrors)
// ============================================================================

const UBUNTU_MIRRORS: &[(&str, &str, &str, &str)] = &[
    // North America
    ("Main (US)", "http://archive.ubuntu.com/ubuntu", "United States", "US"),
    ("US Canonical", "http://us.archive.ubuntu.com/ubuntu", "United States", "US"),
    ("MIT", "http://mirrors.mit.edu/ubuntu", "United States", "US"),
    ("Kernel.org", "http://mirrors.kernel.org/ubuntu", "United States", "US"),
    ("Canada", "http://ca.archive.ubuntu.com/ubuntu", "Canada", "CA"),
    ("Mexico", "http://mx.archive.ubuntu.com/ubuntu", "Mexico", "MX"),
    
    // South America
    ("Brazil", "http://br.archive.ubuntu.com/ubuntu", "Brazil", "BR"),
    ("Brazil USP", "http://mirror.usp.br/ubuntu", "Brazil", "BR"),
    ("Argentina", "http://ar.archive.ubuntu.com/ubuntu", "Argentina", "AR"),
    ("Chile", "http://cl.archive.ubuntu.com/ubuntu", "Chile", "CL"),
    ("Colombia", "http://co.archive.ubuntu.com/ubuntu", "Colombia", "CO"),
    ("Peru", "http://pe.archive.ubuntu.com/ubuntu", "Peru", "PE"),
    ("Venezuela", "http://ve.archive.ubuntu.com/ubuntu", "Venezuela", "VE"),
    ("Ecuador", "http://ec.archive.ubuntu.com/ubuntu", "Ecuador", "EC"),
    
    // Europe
    ("Germany", "http://de.archive.ubuntu.com/ubuntu", "Germany", "DE"),
    ("Germany FAU", "http://ftp.fau.de/ubuntu", "Germany", "DE"),
    ("France", "http://fr.archive.ubuntu.com/ubuntu", "France", "FR"),
    ("France Free", "http://ftp.free.fr/mirrors/ftp.ubuntu.com/ubuntu", "France", "FR"),
    ("UK", "http://gb.archive.ubuntu.com/ubuntu", "United Kingdom", "GB"),
    ("UK Bytemark", "http://mirror.bytemark.co.uk/ubuntu", "United Kingdom", "GB"),
    ("Netherlands", "http://nl.archive.ubuntu.com/ubuntu", "Netherlands", "NL"),
    ("Sweden", "http://se.archive.ubuntu.com/ubuntu", "Sweden", "SE"),
    ("Italy", "http://it.archive.ubuntu.com/ubuntu", "Italy", "IT"),
    ("Spain", "http://es.archive.ubuntu.com/ubuntu", "Spain", "ES"),
    ("Poland", "http://pl.archive.ubuntu.com/ubuntu", "Poland", "PL"),
    ("Russia", "http://ru.archive.ubuntu.com/ubuntu", "Russia", "RU"),
    ("Russia Yandex", "http://mirror.yandex.ru/ubuntu", "Russia", "RU"),
    ("Czech", "http://cz.archive.ubuntu.com/ubuntu", "Czech Republic", "CZ"),
    ("Switzerland", "http://ch.archive.ubuntu.com/ubuntu", "Switzerland", "CH"),
    ("Austria", "http://at.archive.ubuntu.com/ubuntu", "Austria", "AT"),
    ("Belgium", "http://be.archive.ubuntu.com/ubuntu", "Belgium", "BE"),
    ("Denmark", "http://dk.archive.ubuntu.com/ubuntu", "Denmark", "DK"),
    ("Finland", "http://fi.archive.ubuntu.com/ubuntu", "Finland", "FI"),
    ("Norway", "http://no.archive.ubuntu.com/ubuntu", "Norway", "NO"),
    ("Portugal", "http://pt.archive.ubuntu.com/ubuntu", "Portugal", "PT"),
    ("Ireland", "http://ie.archive.ubuntu.com/ubuntu", "Ireland", "IE"),
    ("Greece", "http://gr.archive.ubuntu.com/ubuntu", "Greece", "GR"),
    ("Hungary", "http://hu.archive.ubuntu.com/ubuntu", "Hungary", "HU"),
    ("Romania", "http://ro.archive.ubuntu.com/ubuntu", "Romania", "RO"),
    ("Bulgaria", "http://bg.archive.ubuntu.com/ubuntu", "Bulgaria", "BG"),
    ("Ukraine", "http://ua.archive.ubuntu.com/ubuntu", "Ukraine", "UA"),
    ("Turkey", "http://tr.archive.ubuntu.com/ubuntu", "Turkey", "TR"),
    ("Slovakia", "http://sk.archive.ubuntu.com/ubuntu", "Slovakia", "SK"),
    ("Slovenia", "http://si.archive.ubuntu.com/ubuntu", "Slovenia", "SI"),
    ("Croatia", "http://hr.archive.ubuntu.com/ubuntu", "Croatia", "HR"),
    ("Serbia", "http://rs.archive.ubuntu.com/ubuntu", "Serbia", "RS"),
    ("Estonia", "http://ee.archive.ubuntu.com/ubuntu", "Estonia", "EE"),
    ("Latvia", "http://lv.archive.ubuntu.com/ubuntu", "Latvia", "LV"),
    ("Lithuania", "http://lt.archive.ubuntu.com/ubuntu", "Lithuania", "LT"),
    
    // Asia Pacific
    ("Indonesia", "http://id.archive.ubuntu.com/ubuntu", "Indonesia", "ID"),
    ("Biznet", "http://mirror.biznetgio.com/ubuntu", "Indonesia", "ID"),
    ("Poliwangi", "http://mirror.poliwangi.ac.id/ubuntu", "Indonesia", "ID"),
    ("Telkom", "http://kartolo.sby.datautama.net.id/ubuntu", "Indonesia", "ID"),
    ("Singapore", "http://sg.archive.ubuntu.com/ubuntu", "Singapore", "SG"),
    ("Japan", "http://jp.archive.ubuntu.com/ubuntu", "Japan", "JP"),
    ("JAIST", "http://ftp.jaist.ac.jp/pub/Linux/ubuntu", "Japan", "JP"),
    ("South Korea", "http://kr.archive.ubuntu.com/ubuntu", "South Korea", "KR"),
    ("Kakao", "http://mirror.kakao.com/ubuntu", "South Korea", "KR"),
    ("Australia", "http://au.archive.ubuntu.com/ubuntu", "Australia", "AU"),
    ("Internode", "http://mirror.internode.on.net/pub/ubuntu/ubuntu", "Australia", "AU"),
    ("New Zealand", "http://nz.archive.ubuntu.com/ubuntu", "New Zealand", "NZ"),
    ("India", "http://in.archive.ubuntu.com/ubuntu", "India", "IN"),
    ("IIT Kanpur", "http://mirror.cse.iitk.ac.in/ubuntu", "India", "IN"),
    ("Taiwan", "http://tw.archive.ubuntu.com/ubuntu", "Taiwan", "TW"),
    ("Hong Kong", "http://hk.archive.ubuntu.com/ubuntu", "Hong Kong", "HK"),
    ("China", "http://cn.archive.ubuntu.com/ubuntu", "China", "CN"),
    ("Aliyun", "http://mirrors.aliyun.com/ubuntu", "China", "CN"),
    ("Tencent", "http://mirrors.cloud.tencent.com/ubuntu", "China", "CN"),
    ("USTC", "http://mirrors.ustc.edu.cn/ubuntu", "China", "CN"),
    ("Tsinghua", "http://mirrors.tuna.tsinghua.edu.cn/ubuntu", "China", "CN"),
    ("Thailand", "http://th.archive.ubuntu.com/ubuntu", "Thailand", "TH"),
    ("Vietnam", "http://vn.archive.ubuntu.com/ubuntu", "Vietnam", "VN"),
    ("Malaysia", "http://my.archive.ubuntu.com/ubuntu", "Malaysia", "MY"),
    ("Philippines", "http://ph.archive.ubuntu.com/ubuntu", "Philippines", "PH"),
    ("Bangladesh", "http://bd.archive.ubuntu.com/ubuntu", "Bangladesh", "BD"),
    ("Pakistan", "http://pk.archive.ubuntu.com/ubuntu", "Pakistan", "PK"),
    ("Kazakhstan", "http://kz.archive.ubuntu.com/ubuntu", "Kazakhstan", "KZ"),
    
    // Middle East
    ("Israel", "http://il.archive.ubuntu.com/ubuntu", "Israel", "IL"),
    ("Iran", "http://ir.archive.ubuntu.com/ubuntu", "Iran", "IR"),
    ("Saudi Arabia", "http://sa.archive.ubuntu.com/ubuntu", "Saudi Arabia", "SA"),
    ("UAE", "http://ae.archive.ubuntu.com/ubuntu", "United Arab Emirates", "AE"),
    
    // Africa
    ("South Africa", "http://za.archive.ubuntu.com/ubuntu", "South Africa", "ZA"),
    ("Kenya", "http://ke.archive.ubuntu.com/ubuntu", "Kenya", "KE"),
    ("Nigeria", "http://ng.archive.ubuntu.com/ubuntu", "Nigeria", "NG"),
    ("Egypt", "http://eg.archive.ubuntu.com/ubuntu", "Egypt", "EG"),
    ("Morocco", "http://ma.archive.ubuntu.com/ubuntu", "Morocco", "MA"),
    ("Tunisia", "http://tn.archive.ubuntu.com/ubuntu", "Tunisia", "TN"),
    ("Algeria", "http://dz.archive.ubuntu.com/ubuntu", "Algeria", "DZ"),
];

const AVAILABLE_REGIONS: &[(&str, &str)] = &[
    // Americas
    ("US", "United States"),
    ("CA", "Canada"),
    ("MX", "Mexico"),
    ("BR", "Brazil"),
    ("AR", "Argentina"),
    ("CL", "Chile"),
    ("CO", "Colombia"),
    ("PE", "Peru"),
    ("VE", "Venezuela"),
    ("EC", "Ecuador"),
    
    // Europe
    ("DE", "Germany"),
    ("FR", "France"),
    ("GB", "United Kingdom"),
    ("NL", "Netherlands"),
    ("SE", "Sweden"),
    ("IT", "Italy"),
    ("ES", "Spain"),
    ("PL", "Poland"),
    ("RU", "Russia"),
    ("CZ", "Czech Republic"),
    ("CH", "Switzerland"),
    ("AT", "Austria"),
    ("BE", "Belgium"),
    ("DK", "Denmark"),
    ("FI", "Finland"),
    ("NO", "Norway"),
    ("PT", "Portugal"),
    ("IE", "Ireland"),
    ("GR", "Greece"),
    ("HU", "Hungary"),
    ("RO", "Romania"),
    ("BG", "Bulgaria"),
    ("UA", "Ukraine"),
    ("TR", "Turkey"),
    ("SK", "Slovakia"),
    ("SI", "Slovenia"),
    ("HR", "Croatia"),
    ("RS", "Serbia"),
    ("EE", "Estonia"),
    ("LV", "Latvia"),
    ("LT", "Lithuania"),
    
    // Asia Pacific
    ("ID", "Indonesia"),
    ("SG", "Singapore"),
    ("JP", "Japan"),
    ("KR", "South Korea"),
    ("AU", "Australia"),
    ("NZ", "New Zealand"),
    ("IN", "India"),
    ("TW", "Taiwan"),
    ("HK", "Hong Kong"),
    ("CN", "China"),
    ("TH", "Thailand"),
    ("VN", "Vietnam"),
    ("MY", "Malaysia"),
    ("PH", "Philippines"),
    ("BD", "Bangladesh"),
    ("PK", "Pakistan"),
    ("KZ", "Kazakhstan"),
    
    // Middle East
    ("IL", "Israel"),
    ("IR", "Iran"),
    ("SA", "Saudi Arabia"),
    ("AE", "United Arab Emirates"),
    
    // Africa
    ("ZA", "South Africa"),
    ("KE", "Kenya"),
    ("NG", "Nigeria"),
    ("EG", "Egypt"),
    ("MA", "Morocco"),
    ("TN", "Tunisia"),
    ("DZ", "Algeria"),
];

// ============================================================================
// Helper Functions
// ============================================================================

/// Parse a single line from sources.list
fn parse_repo_line(line: &str, file_path: &str, line_number: usize) -> Option<Repository> {
    let trimmed = line.trim();
    
    if trimmed.is_empty() || (trimmed.starts_with('#') && !trimmed.contains("deb")) {
        return None;
    }
    
    let is_enabled = !trimmed.starts_with('#');
    let clean_line = trimmed.trim_start_matches('#').trim();
    
    let parts: Vec<&str> = clean_line.split_whitespace().collect();
    if parts.len() < 4 {
        return None;
    }
    
    let repo_type = parts[0].to_string();
    if repo_type != "deb" && repo_type != "deb-src" {
        return None;
    }
    
    let (uri_idx, uri) = if parts[1].starts_with('[') {
        let mut idx = 1;
        while idx < parts.len() && !parts[idx].contains(']') {
            idx += 1;
        }
        (idx + 1, parts.get(idx + 1).unwrap_or(&"").to_string())
    } else {
        (1, parts[1].to_string())
    };
    
    if uri_idx + 2 > parts.len() {
        return None;
    }
    
    let suite = parts[uri_idx + 1].to_string();
    let components: Vec<String> = parts[uri_idx + 2..].iter().map(|s| s.to_string()).collect();
    
    let is_ppa = uri.contains("ppa.launchpad.net") || uri.contains("ppa.launchpadcontent.net");
    
    // Extract PPA name from URI
    let ppa_name = if is_ppa {
        // Format: https://ppa.launchpadcontent.net/user/repo/ubuntu
        uri.split('/').skip(3).take(2).collect::<Vec<&str>>().join("/")
            .split("/ubuntu").next()
            .map(|s| format!("ppa:{}", s))
    } else {
        None
    };
    
    Some(Repository {
        file_path: file_path.to_string(),
        line_number,
        repo_type,
        uri,
        suite,
        components,
        is_enabled,
        is_ppa,
        raw_line: line.to_string(),
        ppa_name,
    })
}

/// Parse all repositories from a file
fn parse_sources_file(path: &Path) -> Vec<Repository> {
    let mut repos = Vec::new();
    
    if let Ok(content) = fs::read_to_string(path) {
        for (idx, line) in content.lines().enumerate() {
            if let Some(repo) = parse_repo_line(line, &path.to_string_lossy(), idx + 1) {
                repos.push(repo);
            }
        }
    }
    
    repos
}

/// Detect system region from locale
fn detect_region() -> (String, String) {
    // Try multiple sources
    let locale = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .unwrap_or_else(|_| "en_US.UTF-8".to_string());
    
    // Extract country code from locale (e.g., en_US.UTF-8 -> US)
    let code = locale
        .split('_')
        .nth(1)
        .and_then(|s| s.split('.').next())
        .unwrap_or("US")
        .to_uppercase();
    
    let name = AVAILABLE_REGIONS
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, n)| n.to_string())
        .unwrap_or_else(|| "United States".to_string());
    
    (code, name)
}

// ============================================================================
// Tauri Commands (All async)
// ============================================================================

/// Get all APT repositories
#[tauri::command]
pub async fn get_repositories() -> Result<Vec<Repository>> {
    let repos = tokio::task::spawn_blocking(|| {
        let mut all_repos = Vec::new();
        
        let main_sources = Path::new("/etc/apt/sources.list");
        if main_sources.exists() {
            all_repos.extend(parse_sources_file(main_sources));
        }
        
        let sources_d = Path::new("/etc/apt/sources.list.d");
        if sources_d.exists() {
            if let Ok(entries) = fs::read_dir(sources_d) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map(|e| e == "list").unwrap_or(false) {
                        all_repos.extend(parse_sources_file(&path));
                    }
                }
            }
        }
        
        all_repos
    }).await.unwrap();
    
    Ok(repos)
}

/// Delete a repository file or line
#[tauri::command]
pub async fn delete_repository(file_path: String, is_whole_file: bool) -> Result<String> {
    if is_whole_file {
        // Delete the entire .list file (for PPAs)
        let script = format!("rm -f '{}'", file_path);
        privileged::run_privileged_shell(&script).await?;
        Ok(format!("Deleted {}", file_path))
    } else {
        // Just disable the line (comment it out)
        let content = fs::read_to_string(&file_path)?;
        let lines: Vec<&str> = content.lines().collect();
        let new_content: String = lines.iter()
            .map(|line| {
                if !line.trim().starts_with('#') && line.contains("deb") {
                    format!("# {}", line)
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n") + "\n";
        
        let script = format!(
            "echo '{}' | tee '{}' > /dev/null",
            new_content.replace("'", "'\\''"),
            file_path
        );
        privileged::run_privileged_shell(&script).await?;
        Ok("Repository disabled".to_string())
    }
}

/// Toggle repository enabled/disabled
#[tauri::command]
pub async fn toggle_repository(file_path: String, line_number: usize) -> Result<()> {
    let content = fs::read_to_string(&file_path)?;
    let lines: Vec<&str> = content.lines().collect();
    
    if line_number == 0 || line_number > lines.len() {
        return Err(AppError::System("Invalid line number".to_string()));
    }
    
    let mut new_lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    let line = &new_lines[line_number - 1];
    
    if line.trim().starts_with('#') {
        new_lines[line_number - 1] = line.trim_start_matches('#').trim_start().to_string();
    } else {
        new_lines[line_number - 1] = format!("# {}", line);
    }
    
    let new_content = new_lines.join("\n") + "\n";
    
    let script = format!(
        "echo '{}' | tee '{}' > /dev/null",
        new_content.replace("'", "'\\''"),
        file_path
    );
    privileged::run_privileged_shell(&script).await?;
    
    Ok(())
}

/// Add a PPA
#[tauri::command]
pub async fn add_ppa(ppa: String) -> Result<String> {
    if !ppa.starts_with("ppa:") {
        return Err(AppError::System("Invalid PPA format. Use ppa:user/repo".to_string()));
    }
    
    privileged::run_privileged("add-apt-repository", &["-y", &ppa]).await
}

/// Remove a PPA
#[tauri::command]
pub async fn remove_ppa(ppa: String) -> Result<String> {
    if !ppa.starts_with("ppa:") {
        return Err(AppError::System("Invalid PPA format".to_string()));
    }
    
    privileged::run_privileged("add-apt-repository", &["-r", "-y", &ppa]).await
}

/// Get region info
#[tauri::command]
pub fn get_region_info() -> RegionInfo {
    let (code, name) = detect_region();
    
    RegionInfo {
        detected_country: name,
        detected_code: code,
        available_regions: AVAILABLE_REGIONS.iter().map(|(c, n)| (c.to_string(), n.to_string())).collect(),
    }
}

/// Get mirrors for a specific region (or all if no region specified)
#[tauri::command]
pub fn get_mirrors(region: Option<String>) -> Vec<MirrorInfo> {
    let os_release = fs::read_to_string("/etc/os-release").unwrap_or_default();
    let is_ubuntu = os_release.contains("ubuntu") || os_release.contains("Ubuntu");
    
    if !is_ubuntu {
        return vec![]; // For now only Ubuntu mirrors
    }
    
    UBUNTU_MIRRORS
        .iter()
        .filter(|(_, _, _, code)| {
            region.as_ref().map_or(true, |r| *code == r.as_str() || r == "ALL")
        })
        .map(|(name, uri, country, code)| MirrorInfo {
            name: name.to_string(),
            uri: uri.to_string(),
            country: country.to_string(),
            country_code: code.to_string(),
            latency_ms: None,
        })
        .collect()
}

/// Test a single mirror speed
#[tauri::command]
pub async fn test_mirror_speed(uri: String) -> Result<u64> {
    let start = Instant::now();
    
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| AppError::Network(format!("Failed to create HTTP client: {}", e)))?;
    
    let result = timeout(
        Duration::from_secs(5),
        client.head(&uri).send()
    ).await;
    
    match result {
        Ok(Ok(_)) => Ok(start.elapsed().as_millis() as u64),
        Ok(Err(e)) => Err(AppError::Network(format!("Mirror unreachable: {}", e))),
        Err(_) => Err(AppError::Timeout("Mirror test timed out".to_string())),
    }
}

/// Test mirrors for a region in parallel
#[tauri::command]
pub async fn test_all_mirrors(region: Option<String>) -> Vec<MirrorInfo> {
    let mut mirrors = get_mirrors(region);
    
    let test_futures: Vec<_> = mirrors.iter().map(|m| {
        let uri = m.uri.clone();
        async move {
            test_mirror_speed(uri).await.ok()
        }
    }).collect();
    
    let results = join_all(test_futures).await;
    
    for (mirror, latency) in mirrors.iter_mut().zip(results) {
        mirror.latency_ms = latency;
    }
    
    mirrors.sort_by(|a, b| {
        match (a.latency_ms, b.latency_ms) {
            (Some(a_ms), Some(b_ms)) => a_ms.cmp(&b_ms),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });
    
    mirrors
}

/// Set the fastest mirror as primary
#[tauri::command]
pub async fn set_mirror(new_uri: String) -> Result<String> {
    let sources_path = "/etc/apt/sources.list";
    let content = fs::read_to_string(sources_path)?;
    
    // Replace all known mirror patterns
    let mut new_content = content.clone();
    for (_, uri, _, _) in UBUNTU_MIRRORS {
        new_content = new_content.replace(*uri, &new_uri);
    }
    
    if new_content == content {
        return Ok("No changes needed".to_string());
    }
    
    let script = format!(
        "echo '{}' | tee '{}' > /dev/null",
        new_content.replace("'", "'\\''"),
        sources_path
    );
    privileged::run_privileged_shell(&script).await?;
    
    Ok(format!("Mirror changed to {}", new_uri))
}

// ============================================================================
// apt-fast Integration
// ============================================================================

/// Check if apt-fast is installed
#[tauri::command]
pub fn check_apt_fast() -> AptFastStatus {
    let apt_fast_installed = std::process::Command::new("which")
        .arg("apt-fast")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    
    let aria2_installed = std::process::Command::new("which")
        .arg("aria2c")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    
    // Read max connections from config if exists
    let max_connections = fs::read_to_string("/etc/apt-fast.conf")
        .ok()
        .and_then(|content| {
            content.lines()
                .find(|l| l.starts_with("_MAXNUM="))
                .and_then(|l| l.split('=').nth(1))
                .and_then(|v| v.parse().ok())
        })
        .unwrap_or(5);
    
    AptFastStatus {
        installed: apt_fast_installed,
        aria2_installed,
        max_connections,
    }
}

/// Install apt-fast
#[tauri::command]
pub async fn install_apt_fast() -> Result<String> {
    // Add PPA and install
    let script = r#"
        add-apt-repository -y ppa:apt-fast/stable && 
        apt-get update && 
        DEBIAN_FRONTEND=noninteractive apt-get install -y apt-fast aria2
    "#;
    
    privileged::run_privileged_shell(script).await
}

/// Configure apt-fast max connections
#[tauri::command]
pub async fn configure_apt_fast(max_connections: u32) -> Result<String> {
    let config_content = format!(r#"# apt-fast configuration
_APTMGR=apt-get
_MAXNUM={}
_DOWNLOADER='aria2c --no-conf -c -j ${{_MAXNUM}} -x ${{_MAXNUM}} -s ${{_MAXNUM}} --min-split-size=1M --stream-piece-selector=default -i ${{DLLIST}} --connect-timeout=600 --timeout=600 -m0 --header "Accept: */*"'
"#, max_connections);
    
    let script = format!(
        "echo '{}' | tee /etc/apt-fast.conf > /dev/null",
        config_content
    );
    
    privileged::run_privileged_shell(&script).await?;
    
    Ok(format!("apt-fast configured with {} connections", max_connections))
}

/// Run apt update (with apt-fast if available)
#[tauri::command]
pub async fn apt_update() -> Result<String> {
    let status = check_apt_fast();
    
    if status.installed {
        privileged::run_privileged("apt-fast", &["update"]).await
    } else {
        privileged::run_privileged("apt-get", &["update"]).await
    }
}
