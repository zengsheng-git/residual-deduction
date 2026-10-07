<script setup lang="ts">
import type { UploadFileInfo } from "naive-ui";
import { useMessage } from "naive-ui";
import { NAlert, NButton, NCard, NForm, NFormItem, NInputNumber, NProgress, NRadioButton, NRadioGroup, NSelect, NSlider, NSpace, NSwitch, NTag, NUpload, NUploadDragger } from "naive-ui";
import { onMounted, onUnmounted, ref } from "vue";

import {
    convertFileSrc,
    generateVideo,
    getSettings,
    listenDone,
    listenError,
    listenProgress,
    recognizeImage,
    stopGeneration,
    VOICES,
} from "../api";
import { studioStore as store, resetGenerationState } from "../store";

const message = useMessage();
const uploadList = ref<UploadFileInfo[]>([]);
const voice = ref(VOICES[0].value);
const depth = ref(24);
const movetime = ref(3000);
const branchMax = ref(4);
const rate = ref(0);
const voiceEnabled = ref(true);
const branchesEnabled = ref(true);

const stages: Record<string, string> = {
    analyse: "引擎推演",
    tts: "配音",
    render: "渲染",
    compose: "合成",
    done: "完成",
};

let unlisteners: (() => void)[] = [];

onMounted(async () => {
    // 参数默认值取自设置页
    try {
        const s = await getSettings();
        voice.value = s.voice;
        depth.value = s.depth;
        movetime.value = s.movetime;
        branchMax.value = s.branch_max;
        rate.value = s.rate;
        voiceEnabled.value = s.voice_enabled;
        branchesEnabled.value = s.branches_enabled;
    } catch {
        // 设置读取失败时使用内置默认值
    }

    unlisteners.push(
        await listenProgress((p) => {
            store.progress = p;
            store.logs.push(`[${stages[p.stage] ?? p.stage}] ${p.message}`);
            if (store.logs.length > 120) store.logs.shift();
            // 进度完成即解除生成状态(与 gen://done 事件双保险)
            if (p.stage === "done") {
                store.busy = false;
            }
        }),
        await listenDone((meta) => {
            store.busy = false;
            store.lastVideo = meta;
            store.lastError = "";
            message.success(`视频生成完成: ${meta.title}`);
        }),
        await listenError((err) => {
            store.busy = false;
            store.lastError = err;
            message.error(err);
        }),
    );
});

onUnmounted(() => {
    unlisteners.forEach((fn) => fn());
    unlisteners = [];
});

async function handleUpload({ file }: { file: UploadFileInfo }) {
    // 只处理新加入的文件(清空列表引发的 removed 事件不处理)
    if (!file.file || file.status !== "pending") return;
    const buf = new Uint8Array(await file.file.arrayBuffer());
    try {
        const rec = await recognizeImage(buf);
        store.recognition = rec;
        store.side = "w";
        if (!rec.legal) {
            message.warning(rec.issues[0] ?? "识别结果可能存在误差");
        } else {
            message.success(`识别成功, 共 ${rec.pieces_count} 枚棋子, 请确认先行方`);
        }
    } catch (e) {
        store.recognition = null;
        message.error(String(e));
    } finally {
        // 清空内部列表, 保证随时可以重新上传替换
        uploadList.value = [];
    }
}

async function handleGenerate() {
    const rec = store.recognition;
    if (!rec) return;
    resetGenerationState();
    store.busy = true;
    store.lastVideo = null;
    store.lastError = "";
    try {
        await generateVideo({
            pieces: rec.board,
            side: store.side,
            depth: depth.value,
            movetime: movetime.value,
            branch_max: branchMax.value,
            voice: voice.value,
            rate: rate.value,
            voice_enabled: voiceEnabled.value,
            branches_enabled: branchesEnabled.value,
        });
    } catch (e) {
        store.busy = false;
        message.error(String(e));
    }
}

async function handleStop() {
    await stopGeneration();
    message.info("正在停止生成...");
}

function fmtSize(bytes: number): string {
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function fmtDuration(secs: number): string {
    const m = Math.floor(secs / 60);
    const s = Math.round(secs % 60);
    return `${m}分${s.toString().padStart(2, "0")}秒`;
}
</script>

<template>
  <div class="studio">
    <div class="left-col">
      <div class="left-scroll">
        <n-card title="1 · 上传残局截图" size="small">
          <n-upload
            accept="image/*"
            v-model:file-list="uploadList"
            :max="1"
            :default-upload="true"
            :show-file-list="false"
            @change="handleUpload"
          >
            <n-upload-dragger>
              <div class="upload-hint">
                <div class="upload-icon">♟</div>
                <div>点击或拖入截图 (对局平台 / 棋谱网站 / App 残局页)</div>
                <div class="upload-sub">{{ store.recognition ? "已识别 — 传错了? 重新上传将自动替换" : "系统将自动识别棋盘与棋子" }}</div>
              </div>
            </n-upload-dragger>
          </n-upload>
        </n-card>

        <n-card title="2 · 确认局面并选择生成项" size="small" class="mt">
          <template v-if="store.recognition">
            <div class="preview-row">
              <img v-if="store.recognition.preview_base64" class="board-preview" :src="`data:image/png;base64,${store.recognition.preview_base64}`" alt="识别预览" />
              <div class="preview-meta">
                <n-space vertical>
                  <n-tag :type="store.recognition.legal ? 'success' : 'warning'">
                    {{ store.recognition.legal ? "布局合法" : "布局待核对" }}
                  </n-tag>
                  <span>棋子: {{ store.recognition.pieces_count }} 枚</span>
                  <n-alert v-for="issue in store.recognition.issues" :key="issue" type="warning" :show-icon="false">
                    {{ issue }}
                  </n-alert>
                  <div class="side-pick">
                    <div class="side-label">谁先走?(残局截图无法自动判断)</div>
                    <n-radio-group v-model:value="store.side">
                      <n-radio-button value="w">红方先行</n-radio-button>
                      <n-radio-button value="b">黑方先行</n-radio-button>
                    </n-radio-group>
                  </div>
                  <div class="side-pick">
                    <div class="side-label">视频里要包含什么?</div>
                    <div class="switch-row">
                      <div class="switch-item">
                        <n-switch v-model:value="voiceEnabled" size="small" />
                        <span>语音解说</span>
                      </div>
                      <div class="switch-item">
                        <n-switch v-model:value="branchesEnabled" size="small" />
                        <span>分支推演</span>
                      </div>
                    </div>
                  </div>
                </n-space>
              </div>
            </div>
          </template>
          <template v-else>
            <div class="placeholder">上传截图后自动识别, 在这里确认局面、选先行方和视频内容, 然后生成。</div>
          </template>
        </n-card>

        <n-card title="3 · 高级参数(有默认值, 一般不用动)" size="small" class="mt">
          <n-form label-placement="left" label-width="96">
            <n-form-item label="解说声音">
              <n-select v-model:value="voice" :options="VOICES" :disabled="!voiceEnabled" />
            </n-form-item>
            <n-form-item label="语速">
              <n-slider v-model:value="rate" :min="-30" :max="80" :step="5" :disabled="!voiceEnabled" :format-tooltip="(v: number) => `${v > 0 ? '+' : ''}${v}%`" />
            </n-form-item>
            <n-form-item label="搜索深度">
              <n-input-number v-model:value="depth" :min="12" :max="40" />
            </n-form-item>
            <n-form-item label="每步时限">
              <n-input-number v-model:value="movetime" :min="500" :max="10000" :step="500">
                <template #suffix>ms</template>
              </n-input-number>
            </n-form-item>
            <n-form-item label="分支节点">
              <n-input-number v-model:value="branchMax" :min="0" :max="6" :disabled="!branchesEnabled" />
            </n-form-item>
          </n-form>
        </n-card>
      </div>

      <!-- 底部操作条: 固定在左栏底部, 不随内容滚动 -->
      <div class="action-bar">
        <n-button
          type="primary"
          size="large"
          class="generate-btn"
          :disabled="!store.recognition || store.busy"
          @click="handleGenerate"
        >
          {{ store.busy ? "生成中..." : store.recognition ? "🎬 生成解说视频" : "① 请先上传截图" }}
        </n-button>
        <n-button v-if="store.busy" size="large" @click="handleStop">停止</n-button>
        <span v-if="store.recognition && !store.busy" class="action-hint">点击开始推演与合成, 完成后自动存入视频库</span>
        <span v-if="store.busy" class="action-hint">可切到右侧查看进度</span>
      </div>
    </div>

    <div class="right-col">
      <n-card v-if="store.lastVideo" title="最新成片" size="small">
        <video class="video-player" :src="convertFileSrc(store.lastVideo.video_path)" controls :poster="convertFileSrc(store.lastVideo.thumb_path)" />
        <div class="video-meta">
          <b>{{ store.lastVideo.title }}</b>
          <span>{{ store.lastVideo.verdict }} · {{ fmtDuration(store.lastVideo.duration_secs) }} · {{ fmtSize(store.lastVideo.size_bytes) }}</span>
          <span class="action-hint">也可在"视频库"标签页查看全部成片</span>
        </div>
      </n-card>

      <n-card v-if="store.lastError" title="最近一次失败" size="small" class="mt">
        <n-alert type="error">{{ store.lastError }}</n-alert>
      </n-card>

      <n-card title="生成进度" size="small" :class="{ mt: store.lastVideo || store.lastError }">
        <template v-if="store.progress || store.busy">
          <n-progress
            type="line"
            :percentage="store.progress && store.progress.total > 0 ? Math.round((store.progress.current / store.progress.total) * 100) : 10"
            indicator-placement="inside"
          />
          <div class="progress-stage">{{ stages[store.progress?.stage ?? ''] ?? '准备中' }}</div>
          <div class="log">
            <div v-for="(line, i) in store.logs" :key="i" class="log-line">{{ line }}</div>
          </div>
        </template>
        <div v-else class="idle-hint">还没有任务。生成过程约 1-3 分钟, 视频会自动存入视频库。</div>
      </n-card>
    </div>
  </div>
</template>
