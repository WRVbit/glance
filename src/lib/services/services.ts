// TypeScript services for services commands
import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// Types
// ============================================================================

export interface ServiceInfo {
    name: string;
    description: string;
    load_state: string;
    active_state: string;
    sub_state: string;
    is_enabled: boolean;
    can_stop: boolean;
    can_restart: boolean;
    category: string;
    memory_mb: number | null;
}

export interface ServiceAction {
    name: string;
    action: string;
    success: boolean;
    message: string;
}

// ============================================================================
// API Functions
// ============================================================================

export async function getServices(): Promise<ServiceInfo[]> {
    return invoke('get_services');
}

export async function searchServices(query: string): Promise<ServiceInfo[]> {
    return invoke('search_services', { query });
}

export async function startService(name: string): Promise<ServiceAction> {
    return invoke('start_service', { name });
}

export async function stopService(name: string): Promise<ServiceAction> {
    return invoke('stop_service', { name });
}

export async function restartService(name: string): Promise<ServiceAction> {
    return invoke('restart_service', { name });
}

export async function enableService(name: string): Promise<ServiceAction> {
    return invoke('enable_service', { name });
}

export async function disableService(name: string): Promise<ServiceAction> {
    return invoke('disable_service', { name });
}
