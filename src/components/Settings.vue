<script setup lang="ts">
import { useMessage } from "naive-ui";
import { NButton, NCard, NForm, NFormItem, NInputNumber, NSelect, NSlider, NSwitch } from "naive-ui";
import { onMounted, ref } from "vue";

import { getSettings, saveSettings, VOICES, type AppSettings } from "../api";
const message = useMessage();
const settings = ref<AppSettings | null>(null);

onMounted(async () => {
    settings.value = await getSettings();
});

async function save() {
    if (!settings.value) return;
    try {
        settings.value = await saveSettings(settings.value);
        message.success("设置已保存");
    } catch (e) {
        message.error(String(e));
    }
}
</script>

<template>
  <div class="settings-page">
    <n-card title="生成设置" size="small" class="settings-card">
      <n-form v-if="settings" label-placement="left" label-width="120">
        <n-form-item label="解说声音">
          <n-select v-model:value="settings.voice" :options="VOICES" />
        </n-form-item>
        <n-form-item label="语速">
          <n-slider v-model:value="settings.rate" :min="-30" :max="80" :step="5" />
        </n-form-item>
        <n-form-item label="搜索深度">
          <n-input-number v-model:value="settings.depth" :min="12" :max="40" />
        </n-form-item>
        <n-form-item label="每步时限(ms)">
          <n-input-number v-model:value="settings.movetime" :min="500" :max="10000" :step="500" />
        </n-form-item>
        <n-form-item label="分支分析节点">
          <n-input-number v-model:value="settings.branch_max" :min="0" :max="6" :disabled="!settings.branches_enabled" />
        </n-form-item>
        <n-form-item label="语音解说">
          <n-switch v-model:value="settings.voice_enabled">
            <template #checked>默认生成配音</template>
            <template #unchecked>默认静音视频</template>
          </n-switch>
        </n-form-item>
        <n-form-item label="分支推演">
          <n-switch v-model:value="settings.branches_enabled">
            <template #checked>默认展示分支分析</template>
            <template #unchecked>默认仅主线</template>
          </n-switch>
        </n-form-item>
        <n-form-item label="完成后提示音">
          <n-switch v-model:value="settings.autoplay_sound" />
        </n-form-item>
      </n-form>
      <template #action>
        <NButton type="primary" @click="save">保存设置</NButton>
      </template>
    </n-card>

    <n-card title="关于" size="small" class="settings-card">
      <div class="about">
        <p>残局推演 · 解说视频生产线 —— 上传一张残局截图, 自动识别局面、引擎推演必胜路线, 产出带解说与分支分析的视频, 让观众明白"为什么这样走能赢"。</p>
        <p>识别: YOLOv8 (ONNX Runtime) · 引擎: Pikafish · 配音: Edge TTS · 合成: ffmpeg</p>
        <p class="tip">提示: 解说配音需要联网; 引擎与识别均在本机运行。</p>
      </div>
    </n-card>
  </div>
</template>
