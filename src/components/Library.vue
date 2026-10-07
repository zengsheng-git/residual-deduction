<script setup lang="ts">
import { useDialog, useMessage } from "naive-ui";
import { NButton, NCard, NEmpty, NSpace, NTag } from "naive-ui";
import { convertFileSrc } from "@tauri-apps/api/core";
import { onMounted, onUnmounted, ref } from "vue";

import { deleteVideo, listenDone, listVideos, openVideosFolder, type VideoMeta } from "../api";

const message = useMessage();
const dialog = useDialog();
const videos = ref<VideoMeta[]>([]);
const playing = ref<VideoMeta | null>(null);

async function refresh() {
    try {
        videos.value = await listVideos();
    } catch (e) {
        message.error(`视频库读取失败: ${e}`);
    }
}

let unlistenDone: (() => void) | null = null;

onMounted(async () => {
    await refresh();
    // 生成完成时若正停留在本页, 实时刷新列表
    unlistenDone = await listenDone(() => {
        refresh();
        message.info("新视频已入库");
    });
});

onUnmounted(() => {
    unlistenDone?.();
    unlistenDone = null;
});

function play(v: VideoMeta) {
    playing.value = v;
}

function fmtSize(bytes: number): string {
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function fmtDuration(secs: number): string {
    const m = Math.floor(secs / 60);
    const s = Math.round(secs % 60);
    return `${m}:${s.toString().padStart(2, "0")}`;
}

function fmtDate(ms: number): string {
    const d = new Date(ms);
    return `${d.getFullYear()}-${(d.getMonth() + 1).toString().padStart(2, "0")}-${d.getDate().toString().padStart(2, "0")} ${d.getHours().toString().padStart(2, "0")}:${d.getMinutes().toString().padStart(2, "0")}`;
}

function remove(v: VideoMeta) {
    dialog.warning({
        title: "删除视频",
        content: `确定删除「${v.title}」吗? 该操作不可恢复。`,
        positiveText: "删除",
        negativeText: "取消",
        onPositiveClick: async () => {
            try {
                await deleteVideo(v.id);
                if (playing.value?.id === v.id) playing.value = null;
                message.success("已删除");
                await refresh();
            } catch (e) {
                message.error(String(e));
            }
        },
    });
}
</script>

<template>
  <div class="library">
    <n-space justify="space-between" align="center" class="toolbar">
      <span class="count">共 {{ videos.length }} 个视频 · 存储在应用数据目录</span>
      <n-space>
        <n-button @click="refresh">刷新</n-button>
        <n-button @click="openVideosFolder()">打开目录</n-button>
      </n-space>
    </n-space>

    <n-card v-if="playing" :title="`正在播放: ${playing.title}`" size="small" class="player-card">
      <video
        class="video-player"
        :src="convertFileSrc(playing.video_path)"
        :poster="convertFileSrc(playing.thumb_path)"
        controls
        autoplay
      />
    </n-card>

    <n-empty v-if="videos.length === 0" description="还没有生成过视频, 去工作台上传一张残局截图吧" class="empty" />

    <div class="grid">
      <n-card v-for="v in videos" :key="v.id" size="small" class="video-card" :title="v.title">
        <template #cover>
          <img class="thumb" :src="convertFileSrc(v.thumb_path)" alt="" @click="play(v)" />
        </template>
        <div class="meta">
          <div class="tag-row">
            <n-tag type="info" size="small">{{ v.verdict }}</n-tag>
            <n-tag v-if="v.polished" type="success" size="small">AI 润色</n-tag>
          </div>
          <div class="meta-lines">
            <span>{{ v.move_count }} 步 · {{ fmtDuration(v.duration_secs) }} · {{ fmtSize(v.size_bytes) }}</span>
            <span class="date">{{ fmtDate(v.created_ms) }}</span>
          </div>
        </div>
        <template #action>
          <n-space>
            <n-button size="small" type="primary" @click="play(v)">播放</n-button>
            <n-button size="small" type="error" ghost @click="remove(v)">删除</n-button>
          </n-space>
        </template>
      </n-card>
    </div>
  </div>
</template>
