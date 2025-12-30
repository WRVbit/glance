// TypeScript services for hosts commands
import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// Types
// ============================================================================

export interface HostEntry {
    line_number: number;
    ip: string;
    hostnames: string[];
    comment: string | null;
    is_enabled: boolean;
    raw_line: string;
}

export interface HostsStats {
    total_entries: number;
    enabled_entries: number;
    blocked_domains: number;
}

// ============================================================================
// API Functions
// ============================================================================

export async function getHosts(): Promise<HostEntry[]> {
    return invoke('get_hosts');
}

export async function getHostsStats(): Promise<HostsStats> {
    return invoke('get_hosts_stats');
}

export async function addHost(ip: string, hostname: string, comment?: string): Promise<void> {
    return invoke('add_host', { ip, hostname, comment: comment || null });
}

export async function removeHost(lineNumber: number): Promise<void> {
    return invoke('remove_host', { lineNumber });
}

export async function toggleHost(lineNumber: number): Promise<void> {
    return invoke('toggle_host', { lineNumber });
}

export async function getBlocklists(): Promise<[string, string][]> {
    return invoke('get_blocklists');
}

export async function importBlocklist(url: string): Promise<number> {
    return invoke('import_blocklist', { url });
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
