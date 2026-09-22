#!/usr/bin/env node
// codex 指纹差分题库 —— 硬题、跨模型横比、高 effort，逼真推理模型多花 token。
// 核心判据不是绝对值，而是「同一中转站下，旗舰模型的推理量/正确率不该低于便宜款」。
// 用法: CODEX_PROBE_BASE_URL=... CODEX_PROBE_API_KEY=... node scripts/fingerprint-battery.mjs "模型1,模型2,..."

import process from 'node:process';

const BASE_URL = (process.env.CODEX_PROBE_BASE_URL || '').replace(/\/+$/, '');
const API_KEY = process.env.CODEX_PROBE_API_KEY;
const EFFORT = process.env.CODEX_PROBE_EFFORT || 'high';
const MODELS = (process.argv[2] || process.env.CODEX_PROBE_MODELS || '').split(',').map(s => s.trim()).filter(Boolean);

if (!BASE_URL || !API_KEY || !MODELS.length) {
  console.error('需要 CODEX_PROBE_BASE_URL + CODEX_PROBE_API_KEY，以及参数里传逗号分隔的模型列表');
  process.exit(1);
}

// 全是我确认过标准答案的可判分硬题；末尾要求 "ANSWER: <整数>" 便于解析
const SUFFIX = ' Think step by step, then end your reply with a line exactly like "ANSWER: <integer>".';
const BATTERY = [
  { id: 'P1-inclexcl', a: 266,  q: 'How many positive integers less than 1000 are divisible by none of 2, 3, or 5?' },
  { id: 'P2-modexp',   a: 9,    q: 'Compute 7^100 mod 13.' },
  { id: 'P3-sigma',    a: 1170, q: 'What is the sum of all positive divisors of 360 (including 1 and 360)?' },
  { id: 'P4-cube',     a: 12,   q: 'A 3x3x3 cube is painted on all six outer faces, then cut into 27 unit cubes. How many unit cubes have exactly two painted faces?' },
  { id: 'P5-coins',    a: 12,   q: 'In how many distinct ways can you make 25 cents using only pennies (1c), nickels (5c), and dimes (10c)?' },
];

function parseAnswer(text) {
  const m = text.match(/ANSWER:\s*(-?\d+)/i);
  if (m) return Number(m[1]);
  const nums = text.match(/-?\d+/g);            // 兜底：取最后一个整数
  return nums ? Number(nums[nums.length - 1]) : null;
}

async function ask(model, question) {
  const t0 = performance.now();
  const res = await fetch(`${BASE_URL}/responses`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', authorization: `Bearer ${API_KEY}` },
    body: JSON.stringify({ model, input: question + SUFFIX, reasoning: { effort: EFFORT }, store: false }),
  });
  const ms = Math.round(performance.now() - t0);
  const bodyText = await res.text();
  if (!res.ok) return { ok: false, status: res.status, ms, err: bodyText.slice(0, 160) };
  const j = JSON.parse(bodyText);
  const text = (j.output_text
    || (j.output || []).flatMap(o => (o.content || []).map(c => c.text || '')).join('') || '').trim();
  return {
    ok: true, ms,
    rt: j.usage?.output_tokens_details?.reasoning_tokens ?? null,
    ot: j.usage?.output_tokens ?? null,
    ans: parseAnswer(text),
    reported: j.model,
  };
}

console.log(`题库=${BATTERY.length}题  effort=${EFFORT}  端点=${BASE_URL}\n模型: ${MODELS.join(', ')}\n`);
const summary = {};

for (const model of MODELS) {
  console.log(`\n===== ${model} =====`);
  const s = { rt: 0, ot: 0, ms: 0, correct: 0, ok: 0, reportedMismatch: false };
  for (const p of BATTERY) {
    process.stdout.write(`  ${p.id.padEnd(12)} ... `);
    try {
      const r = await ask(model, p.q);
      if (!r.ok) { console.log(`HTTP ${r.status} ${r.err}`); continue; }
      const good = r.ans === p.a;
      s.ok++; s.rt += r.rt || 0; s.ot += r.ot || 0; s.ms += r.ms; if (good) s.correct++;
      if (r.reported && r.reported !== model) s.reportedMismatch = r.reported;
      console.log(`rt=${String(r.rt).padStart(4)}  ans=${r.ans} ${good ? '✓' : `✗(应=${p.a})`}  ${r.ms}ms`);
    } catch (e) { console.log(`异常 ${e.message}`); }
  }
  summary[model] = s;
}

console.log('\n\n================ 差分汇总 ================');
console.log('模型'.padEnd(22), 'reasoning累计'.padEnd(14), '正确', '均延迟', '自报model');
for (const [m, s] of Object.entries(summary)) {
  const rtNote = s.rt === 0 && s.ok > 0 ? '(全0!非推理?)' : '';
  console.log(
    m.padEnd(22),
    `${s.rt}${rtNote}`.padEnd(14),
    `${s.correct}/${s.ok}`.padEnd(6),
    `${s.ok ? Math.round(s.ms / s.ok) : 0}ms`.padEnd(8),
    s.reportedMismatch ? `⚠改报为 ${s.reportedMismatch}` : '一致'
  );
}
console.log('\n判据: 若「旗舰/高价名」的 reasoning累计 或 正确率 明显低于同站更便宜的名 → 掺水强嫌疑。');
console.log('      reasoning累计=0 → 该名很可能被路由到非推理模型。');
