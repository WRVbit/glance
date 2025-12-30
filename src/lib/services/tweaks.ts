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
}

export interface TweakCategory {
    id: string;
    name: string;
    icon: string;
    tweaks: Tweak[];
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
