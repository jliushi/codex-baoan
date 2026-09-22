#!/usr/bin/env node
// 本机 Exa 搜索桥接：内置 WebSearch 在本机(美国境外)恒空，Exa MCP 工具又调不动，
// 这里直接用 JSON-RPC 走远程 MCP (https://mcp.exa.ai/mcp)，绕过一切。
// 用法: node scripts/exa-search.mjs "查询词" [numResults]
//       node scripts/exa-search.mjs --fetch "https://url"   # 抓网页
import process from 'node:process';

const ENDPOINT = 'https://mcp.exa.ai/mcp';
const HEADERS_BASE = { 'content-type': 'application/json', accept: 'application/json, text/event-stream' };
let sessionId = null;

// MCP streamable-HTTP 响应可能是 application/json 或 text/event-stream，两种都解析
async function rpc(method, params, isNotification = false) {
  const body = { jsonrpc: '2.0', method, ...(isNotification ? {} : { id: Math.floor(Math.random() * 1e6) }), ...(params ? { params } : {}) };
  const headers = { ...HEADERS_BASE };
  if (sessionId) headers['mcp-session-id'] = sessionId;
  const res = await fetch(ENDPOINT, { method: 'POST', headers, body: JSON.stringify(body) });
  const sid = res.headers.get('mcp-session-id');
  if (sid) sessionId = sid;
  if (isNotification) return null;
  const text = await res.text();
  // 解析 SSE: 收集所有 data: 行，取最后一个含 result/error 的 JSON
  let payloads = [];
  if (text.includes('data:')) {
    for (const line of text.split(/\r?\n/)) {
      const m = line.match(/^data:\s?(.*)$/);
      if (m && m[1].trim()) { try { payloads.push(JSON.parse(m[1])); } catch {} }
    }
  } else {
    try { payloads.push(JSON.parse(text)); } catch { throw new Error(`非JSON响应(HTTP ${res.status}): ${text.slice(0, 200)}`); }
  }
  const hit = payloads.reverse().find(p => p.result || p.error) || payloads[0];
  if (!hit) throw new Error(`空响应(HTTP ${res.status}): ${text.slice(0, 200)}`);
  if (hit.error) throw new Error(`RPC错误: ${JSON.stringify(hit.error)}`);
  return hit.result;
}

async function main() {
  const args = process.argv.slice(2);
  const isFetch = args[0] === '--fetch';
  const query = isFetch ? args[1] : args[0];
  const num = isFetch ? undefined : Number(args[1] || 6);
  if (!query) { console.error('用法: node scripts/exa-search.mjs "查询" [n]  |  --fetch "url"'); process.exit(1); }

  // 1) 握手
  await rpc('initialize', {
    protocolVersion: '2024-11-05',
    capabilities: {},
    clientInfo: { name: 'codex-baoan-exa-bridge', version: '0.1.0' },
  });
  await rpc('notifications/initialized', undefined, true);

  // 2) 调工具
  const toolName = isFetch ? 'web_fetch_exa' : 'web_search_exa';
  const toolArgs = isFetch ? { urls: [query] } : { query, numResults: num };
  const result = await rpc('tools/call', { name: toolName, arguments: toolArgs });

  // 3) 输出 content 里的文本
  const parts = (result?.content || []).map(c => c.text || '').filter(Boolean);
  console.log(parts.join('\n\n') || JSON.stringify(result).slice(0, 2000));
}

main().catch(e => { console.error('失败:', e.message); process.exit(1); });
