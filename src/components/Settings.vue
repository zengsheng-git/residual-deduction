<script setup lang="ts">
import { useMessage } from "naive-ui";
import { NButton, NCard, NForm, NFormItem, NInput, NInputNumber, NSelect, NSlider, NSwitch } from "naive-ui";
import { onMounted, ref } from "vue";

import { getSettings, saveSettings, testPolishConnection, VOICES, type AppSettings } from "../api";
const message = useMessage();
const settings = ref<AppSettings | null>(null);
const testing = ref(false);

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

async function testPolish() {
    if (!settings.value) return;
    testing.value = true;
    try {
        const result = await testPolishConnection(settings.value.polish);
        message.success(`润色接口正常, 模型返回: ${result}`);
    } catch (e) {
        message.error(String(e));
    } finally {
        testing.value = false;
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

    <n-card title="AI 解说润色 (可选)" size="small" class="settings-card">
      <n-form v-if="settings" label-placement="left" label-width="120">
        <n-form-item label="启用润色">
          <n-switch v-model:value="settings.polish.enabled">
            <template #checked>模板生成后交给大模型改写</template>
            <template #unchecked>仅使用模板解说</template>
          </n-switch>
        </n-form-item>
        <n-form-item label="接口地址">
          <n-input v-model:value="settings.polish.base_url" placeholder="OpenAI 兼容接口, 如 https://api.deepseek.com 或 https://api.openai.com/v1" />
        </n-form-item>
        <n-form-item label="API Key">
          <n-input v-model:value="settings.polish.api_key" type="password" show-password-on="click" placeholder="sk-..." />
        </n-form-item>
        <n-form-item label="模型名称">
          <n-input v-model:value="settings.polish.model" placeholder="如 deepseek-chat / gpt-4o-mini" />
        </n-form-item>
        <n-form-item label="测试">
          <n-button size="small" :loading="testing" @click="testPolish">发送示例解说词, 验证连通性</n-button>
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
