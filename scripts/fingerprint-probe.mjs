#!/usr/bin/env node
// codex 指纹原型探针 —— 验证「reasoning_tokens 随 effort 单调 + 延迟同步」这个核心假设
// 零依赖，Node18+ 原生 fetch。不碰主程序，只为决定路线 A 权重。
//
// 用法（任选其一）:
//   1) 显式传参（推荐，最省心）:
//      CODEX_PROBE_BASE_URL="https://中转站/v1" CODEX_PROBE_API_KEY="sk-xxx" \
//      CODEX_PROBE_MODEL="gpt-5-codex" node scripts/fingerprint-probe.mjs
//   2) 不传，自动读 ~/.codex/auth.json 的 key + config.toml 的 base_url/model（best-effort）
//
// 注意: 会向目标端点发真实请求，消耗你的额度。默认每档 effort 跑 2 次。

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const EFFORTS = ['low', 'medium', 'high'];
const RUNS_PER_EFFORT = Number(process.env.CODEX_PROBE_RUNS || 2);

// 一道够难、需要真推理、答案可校验的题。真 codex 应稳过；非推理/量化模型易翻车。
const PROBE_PROMPT =
  'A snail climbs a 30-meter well. Each day it climbs 7 meters, each night it slips back 2 meters. ' +
  'On which day does it first reach the top? Answer with ONLY the integer day number, nothing else.';
// 校验: 第 n 天早晨在 5*(n-1) 米，当天爬到 5*(n-1)+7；≥30 时 n=6。故答案为 6。
const EXPECTED_ANSWER = '6';
const isCorrect = (text) => text.replace(/\D/g, '') === EXPECTED_ANSWER;

// ---- 读取配置 -----------------------------------------------------------
function codexHome() {
  return process.env.CODEX_HOME || path.join(os.homedir(), '.codex');
}

function readKeyFromAuth() {
  try {
    const raw = fs.readFileSync(path.join(codexHome(), 'auth.json'), 'utf8');
    const j = JSON.parse(raw);
    return j.OPENAI_API_KEY || j.openai_api_key || (j.tokens && j.tokens.access_token) || null;
  } catch {
    return null;
  }
}

// 极简 TOML 抓取: 只提 base_url 和顶层 model（够探针用，不追求完整解析）
function readConfigToml() {
  try {
    const raw = fs.readFileSync(path.join(codexHome(), 'config.toml'), 'utf8');
    const baseUrl = (raw.match(/base_url\s*=\s*"([^"]+)"/) || [])[1] || null;
    const model = (raw.match(/^\s*model\s*=\s*"([^"]+)"/m) || [])[1] || null;
    return { baseUrl, model };
  } catch {
    return { baseUrl: null, model: null };
  }
}

const cfg = readConfigToml();
const BASE_URL = (process.env.CODEX_PROBE_BASE_URL || cfg.baseUrl || '').replace(/\/+$/, '');
const API_KEY = process.env.CODEX_PROBE_API_KEY || readKeyFromAuth();
const MODEL = process.env.CODEX_PROBE_MODEL || cfg.model || 'gpt-5-codex';

if (!BASE_URL || !API_KEY) {
  console.error('缺 base_url 或 api_key。请用 CODEX_PROBE_BASE_URL / CODEX_PROBE_API_KEY 显式传入，');
  console.error('或确认 ~/.codex/config.toml 有 base_url、~/.codex/auth.json 有 OPENAI_API_KEY。');
  console.error(`当前解析到: base_url=${BASE_URL || '(空)'}  model=${MODEL}  key=${API_KEY ? '已读到' : '(空)'}`);
  process.exit(1);
}

// ---- 单次探测: 优先 /responses，404 回退 /chat/completions -----------------
async function probeOnce(effort) {
  const t0 = performance.now();

  // 先试 Responses API（codex CLI 的原生路径）
  let usage, text, wire = 'responses';
  let res = await fetch(`${BASE_URL}/responses`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', authorization: `Bearer ${API_KEY}` },
    body: JSON.stringify({
      model: MODEL,
      input: PROBE_PROMPT,
      reasoning: { effort },
      store: false,
    }),
  });

  if (res.status === 404 || res.status === 405) {
    // 中转站只实现了 chat/completions 的情况
    wire = 'chat';
    res = await fetch(`${BASE_URL}/chat/completions`, {
      method: 'POST',
      headers: { 'content-type': 'application/json', authorization: `Bearer ${API_KEY}` },
      body: JSON.stringify({
        model: MODEL,
        messages: [{ role: 'user', content: PROBE_PROMPT }],
        reasoning_effort: effort,
      }),
    });
  }

  const ms = Math.round(performance.now() - t0);
  const bodyText = await res.text();
  if (!res.ok) {
    return { effort, wire, ok: false, status: res.status, ms, err: bodyText.slice(0, 300) };
  }

  const j = JSON.parse(bodyText);
  if (wire === 'responses') {
    usage = j.usage || {};
    const rt = usage.output_tokens_details?.reasoning_tokens ?? null;
    text = (j.output_text
      || (j.output || []).flatMap(o => (o.content || []).map(c => c.text || '')).join('')
      || '').trim();
    return {
      effort, wire, ok: true, status: 200, ms,
      reasoning_tokens: rt,
      output_tokens: usage.output_tokens ?? null,
      reported_model: j.model ?? null,
      service_tier: j.service_tier ?? null,
      answer: text.slice(0, 40),
      correct: isCorrect(text),
    };
  } else {
    usage = j.usage || {};
    const rt = usage.completion_tokens_details?.reasoning_tokens ?? null;
    text = (j.choices?.[0]?.message?.content || '').trim();
    return {
      effort, wire, ok: true, status: 200, ms,
      reasoning_tokens: rt,
      output_tokens: usage.completion_tokens ?? null,
      reported_model: j.model ?? null,
      system_fingerprint: j.system_fingerprint ?? null,
      answer: text.slice(0, 40),
      correct: isCorrect(text),
    };
  }
}

// ---- 主流程 -------------------------------------------------------------
console.log(`探测目标: ${BASE_URL}  model=${MODEL}  每档跑 ${RUNS_PER_EFFORT} 次\n`);

const rows = [];
for (const effort of EFFORTS) {
  for (let i = 0; i < RUNS_PER_EFFORT; i++) {
    process.stdout.write(`  effort=${effort} #${i + 1} ... `);
    try {
      const r = await probeOnce(effort);
      rows.push(r);
      if (r.ok) {
        console.log(`${r.ms}ms  reasoning_tokens=${r.reasoning_tokens}  correct=${r.correct}  wire=${r.wire}`);
      } else {
        console.log(`HTTP ${r.status}  ${r.err}`);
      }
    } catch (e) {
      console.log(`异常: ${e.message}`);
      rows.push({ effort, ok: false, err: e.message });
    }
  }
}

// ---- 汇总 + 启发式判据 ---------------------------------------------------
function avg(arr) { return arr.length ? Math.round(arr.reduce((a, b) => a + b, 0) / arr.length) : null; }
const ok = rows.filter(r => r.ok);
console.log('\n==== 汇总 ====');
for (const effort of EFFORTS) {
  const g = ok.filter(r => r.effort === effort);
  if (!g.length) { console.log(`effort=${effort}: 无成功样本`); continue; }
  const rts = g.map(r => r.reasoning_tokens).filter(x => x != null);
  console.log(
    `effort=${effort.padEnd(6)}  ` +
    `均延迟=${avg(g.map(r => r.ms))}ms  ` +
    `均reasoning_tokens=${rts.length ? avg(rts) : 'N/A'}  ` +
    `正确率=${g.filter(r => r.correct).length}/${g.length}`
  );
}

console.log('\n==== 启发式判据 ====');
const allRt = ok.map(r => r.reasoning_tokens).filter(x => x != null);
const rtByEffort = EFFORTS.map(e => avg(ok.filter(r => r.effort === e).map(r => r.reasoning_tokens).filter(x => x != null)));
const msByEffort = EFFORTS.map(e => avg(ok.filter(r => r.effort === e).map(r => r.ms)));

if (!allRt.length || allRt.every(x => x === 0)) {
  console.log('⚠ reasoning_tokens 全为 0 或缺失 → 强烈怀疑换成了「非推理模型」冒充 codex。');
} else {
  const monoRt = rtByEffort.every((v, i) => i === 0 || v == null || rtByEffort[i - 1] == null || v >= rtByEffort[i - 1]);
  const monoMs = msByEffort.every((v, i) => i === 0 || v == null || msByEffort[i - 1] == null || v >= msByEffort[i - 1]);
  console.log(`reasoning_tokens 随 effort 单调↑: ${monoRt ? '是 ✓（符合真推理模型）' : '否 ✗（可疑：自报数字可能是伪造的静态值）'}`);
  console.log(`延迟随 effort 单调↑:          ${monoMs ? '是 ✓' : '否 ✗（effort 没真正生效？）'}`);
  if (!monoRt || !monoMs) {
    console.log('→ 若 reasoning_tokens 变了但延迟没变，是静态伪造嫌疑；两者都不动则 effort 参数被中转站吞了。');
  }
}
const correctRate = ok.length ? ok.filter(r => r.correct).length / ok.length : 0;
console.log(`能力题正确率: ${(correctRate * 100).toFixed(0)}%  ${correctRate < 0.6 ? '⚠ 偏低，疑降智/量化（需更大题库确认）' : '✓'}`);
