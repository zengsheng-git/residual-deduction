import { reactive } from "vue";

import type { GenProgress, Recognition, VideoMeta } from "./api";

// 工作台共享状态: 识别结果、生成进度、最近成片
export const studioStore = reactive({
    recognition: null as Recognition | null,
    side: "w" as "w" | "b",
    busy: false,
    progress: null as GenProgress | null,
    logs: [] as string[],
    lastVideo: null as VideoMeta | null,
    lastError: "" as string,
});

export function resetGenerationState() {
    studioStore.busy = false;
    studioStore.progress = null;
    studioStore.logs = [];
}

// 开发模式调试钩子: 浏览器里可直接改状态验证 UI(生产构建自动剔除)
if (import.meta.env.DEV) {
    (window as unknown as { __studioStore: typeof studioStore }).__studioStore = studioStore;
}
