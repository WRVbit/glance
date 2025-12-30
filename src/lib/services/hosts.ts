// TypeScript services for Ad-Block Manager
import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// Types
// ============================================================================

export interface BlocklistSource {
    id: string;
    name: string;
    url: string;
    description: string;
    domain_count: number | null;
    is_enabled: boolean;
}

export interface AdBlockStats {
    total_blocked_domains: number;
    active_blocklists: string[];
    hosts_file_size: number;
}

// ============================================================================
// API Functions
// ============================================================================

export async function getBlocklistSources(): Promise<BlocklistSource[]> {
    return invoke('get_blocklist_sources');
}

export async function getAdBlockStats(): Promise<AdBlockStats> {
    return invoke('get_adblock_stats');
}

export async function applyBlocklists(sourceIds: string[]): Promise<number> {
    return invoke('apply_blocklists', { sourceIds });
}

export async function clearBlocklists(): Promise<void> {
    return invoke('clear_blocklists');
}

export async function backupHosts(): Promise<string> {
    return invoke('backup_hosts');
}

export async function listHostsBackups(): Promise<string[]> {
    return invoke('list_hosts_backups');
}

export async function restoreHosts(backupPath: string): Promise<void> {
    return invoke('restore_hosts', { backupPath });
}
