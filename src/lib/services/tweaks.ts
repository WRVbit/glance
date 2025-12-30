// TypeScript services for tweaks commands
import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// Types
// ============================================================================

export interface Tweak {
    id: string;
    name: string;
    category: string;
    description: string;
    current_value: string;
    recommended_value: string;
    is_applied: boolean;
    sysctl_key: string | null;
    file_path: string | null;
    // New fields for enhanced UI
    min_value: number | null;
    max_value: number | null;
    options: string[] | null;
    tweak_type: string; // "slider", "selector", "preset"
}

export interface TweakCategory {
    id: string;
    name: string;
    icon: string;
    tweaks: Tweak[];
}

export interface DeviceInfo {
    tier: string; // "low", "mid", "high"
    ram_gb: number;
    disk_type: string; // "nvme", "ssd", "hdd"
    disk_device: string;
}

// ============================================================================
// API Functions
// ============================================================================

export async function getTweaks(): Promise<TweakCategory[]> {
    return invoke('get_tweaks');
}

export async function applyTweak(tweakId: string, value: string): Promise<string> {
    return invoke('apply_tweak', { tweakId, value });
}

export async function applyAllRecommended(): Promise<string[]> {
    return invoke('apply_all_recommended');
}

export async function getDeviceInfo(): Promise<DeviceInfo> {
    return invoke('get_device_info');
}

// Helper: Format bytes for display
export function formatBufferSize(bytes: number): string {
    if (bytes >= 1048576) {
        return `${(bytes / 1048576).toFixed(0)} MB`;
    } else if (bytes >= 1024) {
        return `${(bytes / 1024).toFixed(0)} KB`;
    }
    return `${bytes} B`;
}

// Helper: Get CPU governor display name
export function getGovernorLabel(gov: string): { icon: string; label: string } {
    switch (gov) {
        case 'powersave':
            return { icon: '🔋', label: 'Power Saver' };
        case 'schedutil':
        case 'ondemand':
        case 'conservative':
            return { icon: '⚖️', label: 'Balanced' };
        case 'performance':
            return { icon: '⚡', label: 'Performance' };
        default:
            return { icon: '⚙️', label: gov };
    }
}
