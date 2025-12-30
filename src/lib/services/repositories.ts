// TypeScript services for repositories commands
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
}

export interface MirrorInfo {
    name: string;
    uri: string;
    country: string;
    latency_ms: number | null;
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

export async function addPpa(ppa: string): Promise<string> {
    return invoke('add_ppa', { ppa });
}

export async function removePpa(ppa: string): Promise<string> {
    return invoke('remove_ppa', { ppa });
}

export async function getMirrors(): Promise<MirrorInfo[]> {
    return invoke('get_mirrors');
}

export async function testMirrorSpeed(uri: string): Promise<number> {
    return invoke('test_mirror_speed', { uri });
}

export async function testAllMirrors(): Promise<MirrorInfo[]> {
    return invoke('test_all_mirrors');
}

export async function setMirror(newUri: string): Promise<string> {
    return invoke('set_mirror', { newUri });
}

export async function aptUpdate(): Promise<string> {
    return invoke('apt_update');
}
