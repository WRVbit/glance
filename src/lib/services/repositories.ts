// TypeScript services for repositories commands - Enhanced
import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// Types
// ============================================================================

export interface Repository {
    file_path: string;
    line_number: number;
    repo_type: string;
    uri: string;
    suite: string;
    components: string[];
    is_enabled: boolean;
    is_ppa: boolean;
    raw_line: string;
    ppa_name: string | null;
}

export interface MirrorInfo {
    name: string;
    uri: string;
    country: string;
    country_code: string;
    latency_ms: number | null;
}

export interface RegionInfo {
    detected_country: string;
    detected_code: string;
    available_regions: [string, string][]; // [code, name]
}

export interface AptFastStatus {
    installed: boolean;
    aria2_installed: boolean;
    max_connections: number;
}

// ============================================================================
// API Functions
// ============================================================================

export async function getRepositories(): Promise<Repository[]> {
    return invoke('get_repositories');
}

export async function toggleRepository(filePath: string, lineNumber: number): Promise<void> {
    return invoke('toggle_repository', { filePath, lineNumber });
}

export async function deleteRepository(filePath: string, isWholeFile: boolean): Promise<string> {
    return invoke('delete_repository', { filePath, isWholeFile });
}

export async function addPpa(ppa: string): Promise<string> {
    return invoke('add_ppa', { ppa });
}

export async function removePpa(ppa: string): Promise<string> {
    return invoke('remove_ppa', { ppa });
}

export async function getRegionInfo(): Promise<RegionInfo> {
    return invoke('get_region_info');
}

export async function getMirrors(region?: string): Promise<MirrorInfo[]> {
    return invoke('get_mirrors', { region: region || null });
}

export async function testMirrorSpeed(uri: string): Promise<number> {
    return invoke('test_mirror_speed', { uri });
}

export async function testAllMirrors(region?: string): Promise<MirrorInfo[]> {
    return invoke('test_all_mirrors', { region: region || null });
}

export async function setMirror(newUri: string): Promise<string> {
    return invoke('set_mirror', { newUri });
}

export async function aptUpdate(): Promise<string> {
    return invoke('apt_update');
}

// apt-fast functions
export async function checkAptFast(): Promise<AptFastStatus> {
    return invoke('check_apt_fast');
}

export async function installAptFast(): Promise<string> {
    return invoke('install_apt_fast');
}

export async function configureAptFast(maxConnections: number): Promise<string> {
    return invoke('configure_apt_fast', { maxConnections });
}
