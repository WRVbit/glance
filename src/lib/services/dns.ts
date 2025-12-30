// TypeScript services for DNS Manager
import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// Types
// ============================================================================

export interface DnsProvider {
    id: string;
    name: string;
    description: string;
    primary_dns: string;
    secondary_dns: string;
    category: string; // "general", "adblock", "security", "family"
}

export interface DnsStatus {
    current_dns: string[];
    active_provider: string | null;
}

// ============================================================================
// API Functions
// ============================================================================

export async function getDnsProviders(): Promise<DnsProvider[]> {
    return invoke('get_dns_providers');
}

export async function getCurrentDns(): Promise<DnsStatus> {
    return invoke('get_current_dns');
}

export async function setDnsProvider(providerId: string): Promise<void> {
    return invoke('set_dns_provider', { providerId });
}

export async function setCustomDns(primary: string, secondary: string): Promise<void> {
    return invoke('set_custom_dns', { primary, secondary });
}

export async function resetDns(): Promise<void> {
    return invoke('reset_dns');
}
