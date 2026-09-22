#!/usr/bin/env node
// 借 sub2api 思路：读上游自报的真实模型。重点看流式各事件的 model 字段，
// 尤其"终结事件"(response.completed/done) —— 中间帧常回显请求名，终帧才带真实模型。
// 用法: CODEX_PROBE_BASE_URL=.../v1 CODEX_PROBE_API_KEY=sk-... node scripts/model-report-probe.mjs 模型名
import process from 'node:process';

const BASE = (process.env.CODEX_PROBE_BASE_URL || '').replace(/\/+$/, '');
const KEY = process.env.CODEX_PROBE_API_KEY;
const MODEL = process.argv[2];
if (!BASE || !KEY || !MODEL) { console.error('需要 BASE_URL / API_KEY / 模型名'); process.exit(1); }

const res = await fetch(`${BASE}/responses`, {
  method: 'POST',
  headers: { 'content-type': 'application/json', authorization: `Bearer ${KEY}`, accept: 'text/event-stream' },
  body: JSON.stringify({ model: MODEL, input: 'Say hi in one word.', stream: true, store: false }),
});

console.log(`请求模型 = ${MODEL}   HTTP ${res.status}\n--- 逐事件 model 字段 ---`);
if (!res.ok || !res.body) { console.log(await res.text()); process.exit(1); }

const dec = new TextDecoder();
let buf = '', curEvent = '', seen = new Map(), terminalModel = null;
const TERMINAL = /^response\.(completed|done|failed|incomplete)$/;

for await (const chunk of res.body) {
  buf += dec.decode(chunk, { stream: true });
  const lines = buf.split(/\r?\n/); buf = lines.pop();
  for (const line of lines) {
    if (line.startsWith('event:')) { curEvent = line.slice(6).trim(); continue; }
    const m = line.match(/^data:\s?(.*)$/); if (!m || !m[1].trim()) continue;
    let j; try { j = JSON.parse(m[1]); } catch { continue; }
    // model 可能在 j.model 或 j.response.model
    const model = j.response?.model ?? j.model ?? null;
    const etype = j.type || curEvent;
    if (model) {
      if (!seen.has(etype)) { seen.set(etype, model); console.log(`  [${etype}] model = ${model}`); }
      if (TERMINAL.test(etype)) terminalModel = model;
    }
  }
}

console.log('\n--- 结论 ---');
const models = [...new Set([...seen.values()])];
console.log(`出现过的 model 值: ${models.join(', ')}`);
console.log(`终结事件自报 model: ${terminalModel ?? '(无终结事件或未带model)'}`);
if (terminalModel && terminalModel !== MODEL) {
  console.log(`⚠ 命中 sub2api 判据: 请求 ${MODEL} 但终帧自报 ${terminalModel} → 被路由/替换`);
} else if (models.length > 1) {
  console.log(`⚠ 多个 model 值冲突 ${models.join(' vs ')} → 中间帧与终帧不一致，可疑`);
} else {
  console.log(`所有事件自报都 = 请求名 → 上游要么诚实、要么把 model 字段改干净了(sub2api 这层看不穿)`);
}
