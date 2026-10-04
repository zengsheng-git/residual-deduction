// 验证 Edge TTS(朗读接口)在本机可用: 合成一句中文并落盘 mp3
import crypto from "node:crypto";
import fs from "node:fs";

const TRUSTED_TOKEN = "6A5AA1D4EAFF4E9FB37E23D68491D6F4";

function secMsGec() {
  // Windows 文件时间(100ns), 向下取整到 5 分钟边界
  let ticks = Math.floor(Date.now() / 1000) + 11644473600;
  ticks -= ticks % 300;
  ticks *= 10_000_000;
  return crypto.createHash("sha256").update(`${ticks}${TRUSTED_TOKEN}`).digest("hex").toUpperCase();
}

function uuidNoDash() {
  return crypto.randomUUID().replaceAll("-", "");
}

const UA =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36 Edg/140.0.0.0";

const ws = new WebSocket(
  `wss://speech.platform.bing.com/consumer/speech/synthesize/readaloud/edge/v1?TrustedClientToken=${TRUSTED_TOKEN}&Sec-MS-GEC=${secMsGec()}&Sec-MS-GEC-Version=1-140.0.3485.54`,
  { headers: { Origin: "chrome-extension://jdiccldimpdaibmpdkjnbmckianbfold", "User-Agent": UA } },
);

const chunks = [];
let turnEnd = false;
const id = uuidNoDash();
const now = () => new Date().toString().replace(/\((.*)\)/, "").trim();

ws.onopen = () => {
  console.log("WS open, sending config + ssml");
  ws.send(
    `X-Timestamp:${now()}\r\nContent-Type:application/json; charset=utf-8\r\nPath:speech.config\r\n\r\n` +
      `{"context":{"synthesis":{"audio":{"metadataoptions":{"sentenceBoundaryEnabled":"false","wordBoundaryEnabled":"true"},"outputFormat":"audio-24khz-48kbitrate-mono-mp3"}}}}`,
  );
  const text = "欢迎收看残局推演。红方车炮对黑方双士, 引擎判断红方四步绝杀, 我们一步一步来看。";
  const ssml =
    `<speak version='1.0' xmlns='http://www.w3.org/2001/10/synthesis' xml:lang='zh-CN'>` +
    `<voice name='zh-CN-YunxiNeural'><prosody rate='+0%' pitch='+0Hz'>${text}</prosody></voice></speak>`;
  ws.send(
    `X-RequestId:${id}\r\nContent-Type:application/ssml+xml\r\nX-Timestamp:${now()}Z\r\nPath:ssml\r\n\r\n${ssml}`,
  );
};

ws.onmessage = async (event) => {
  if (typeof event.data === "string") {
    console.log("TEXT MSG:", event.data.slice(0, 200).replace(/\r\n/g, " | "));
    if (event.data.includes("Path:turn.end")) {
      turnEnd = true;
      console.log("turn.end received, audio bytes:", chunks.reduce((a, b) => a + b.length, 0));
      ws.close();
    }
  } else {
    // binary: [u16 headerLen][headers][audio]
    const buf = Buffer.from(await event.data.arrayBuffer());
    const headerLen = buf.readUInt16BE(0);
    chunks.push(buf.subarray(2 + headerLen));
  }
};

ws.onclose = (e) => {
  console.log("close event code=", e.code, "reason=", e.reason);
};

ws.onerror = (e) => {
  console.error("WS error:", e.message ?? e);
  process.exit(1);
};

ws.onclose = () => {
  const mp3 = Buffer.concat(chunks);
  fs.mkdirSync("scripts/out", { recursive: true });
  fs.writeFileSync("scripts/out/tts-test.mp3", mp3);
  console.log(`saved scripts/out/tts-test.mp3 (${mp3.length} bytes), turnEnd=${turnEnd}`);
  process.exit(turnEnd && mp3.length > 1000 ? 0 : 2);
};

setTimeout(() => {
  console.error("timeout");
  process.exit(3);
}, 30000);
