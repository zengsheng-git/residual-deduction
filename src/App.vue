<script setup lang="ts">
import { NConfigProvider, NDialogProvider, NMessageProvider, NTabPane, NTabs, dateZhCN, darkTheme, zhCN } from "naive-ui";
import { ref } from "vue";

import Library from "./components/Library.vue";
import Settings from "./components/Settings.vue";
import Studio from "./components/Studio.vue";

const tab = ref("studio");
</script>

<template>
  <n-config-provider :theme="darkTheme" :locale="zhCN" :date-locale="dateZhCN" class="root">
    <n-message-provider>
      <n-dialog-provider>
        <div class="app-shell">
          <header class="app-header">
            <div class="brand">
              <span class="logo">残</span>
              <div class="brand-text">
                <div class="brand-title">残局推演 · 解说视频生产线</div>
                <div class="brand-sub">上传残局截图 → 识别局面 → 引擎推演 → 自动产出解说视频</div>
              </div>
            </div>
          </header>
          <main class="app-main">
            <n-tabs v-model:value="tab" type="line" size="large">
              <n-tab-pane name="studio" tab="工作台" display-directive="show"><Studio /></n-tab-pane>
              <n-tab-pane name="library" tab="视频库"><Library /></n-tab-pane>
              <n-tab-pane name="settings" tab="设置"><Settings /></n-tab-pane>
            </n-tabs>
          </main>
        </div>
      </n-dialog-provider>
    </n-message-provider>
  </n-config-provider>
</template>
