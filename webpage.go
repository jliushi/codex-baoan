package main

const indexHTML = `<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Codex 保安 · 模型审计</title>
<style>
:root{
 --bg:#f5f7fa;--panel:#ffffff;--line:#e6ebf1;--ink:#1a2733;--muted:#6b7a8a;
 --blue:#2563eb;--blue-soft:#eaf1fe;--ok:#1a9d63;--ok-soft:#e6f6ee;--warn:#c67c15;--warn-soft:#fdf3e3;--danger:#d63b3b;--danger-soft:#fbeaea;
 --radius:12px;font-family:"Segoe UI","Microsoft YaHei",system-ui,sans-serif}
body[data-theme=dark]{--bg:#0f1720;--panel:#17212e;--line:#26333f;--ink:#e6edf3;--muted:#8b9bab;
 --blue-soft:#16283f;--ok-soft:#123324;--warn-soft:#33280f;--danger-soft:#3a1c1c}
body[data-theme=dark] th{background:#1c2735}
*{box-sizing:border-box}
body{margin:0;background:var(--bg);color:var(--ink);font-size:14px;display:flex;min-height:100vh}
.side{width:214px;flex:0 0 214px;background:#0f1e2e;color:#c2d0dc;display:flex;flex-direction:column;padding:22px 16px}
.brand{display:flex;align-items:center;gap:11px;font-size:15px;font-weight:650;color:#fff;margin-bottom:30px}
.brand .mk{width:34px;height:34px;border-radius:9px;background:linear-gradient(135deg,#2f7cf6,#2bb99a);display:grid;place-items:center;font-size:18px}
.brand small{display:block;font-size:9px;letter-spacing:2px;color:#7f95a6;font-weight:500;margin-top:3px}
.nav{border:0;background:none;color:#aebccb;text-align:left;padding:10px 12px;border-radius:8px;font:inherit;cursor:pointer;display:flex;gap:10px;align-items:center;margin-bottom:4px}
.nav.on,.nav:hover{background:#1b3048;color:#eaf2f9}
.side .foot{margin-top:auto;font-size:11px;color:#7f95a6;line-height:1.8}
.dot{display:inline-block;width:8px;height:8px;border-radius:50%;background:#3fbf83;margin-right:6px}.dot.off{background:#c9954a}
.main{flex:1;min-width:0;display:flex;flex-direction:column}
.top{display:flex;align-items:center;gap:14px;padding:18px 26px;border-bottom:1px solid var(--line);background:var(--panel)}
.top h1{font-size:17px;margin:0;font-weight:650}.top .sub{color:var(--muted);font-size:12px;margin-top:2px}
.spacer{flex:1}
.pill{display:inline-flex;align-items:center;gap:7px;font-size:12px;padding:6px 11px;border-radius:999px;font-weight:600}
.pill.ok{background:var(--ok-soft);color:var(--ok)}.pill.bad{background:var(--warn-soft);color:var(--warn)}
input[type=date]{background:var(--panel);border:1px solid var(--line);color:var(--ink);border-radius:9px;padding:7px 10px;font:inherit}
.btn{background:var(--blue);border:0;color:#fff;border-radius:9px;padding:8px 15px;font:inherit;font-weight:600;cursor:pointer}
.btn:hover{filter:brightness(1.06)}.btn.ghost{background:var(--panel);border:1px solid var(--line);color:#33475b;font-weight:500}
.btn:disabled{opacity:.5;cursor:default}
.body{padding:22px 26px;overflow:auto}
.setup{background:var(--blue-soft);border:1px solid #d3e2fb;border-radius:var(--radius);padding:14px 16px;margin-bottom:18px;display:flex;gap:18px;align-items:center;flex-wrap:wrap}
.setup .step{font-size:12.5px;color:#2b4a74}.setup code{background:#fff;border:1px solid #d3e2fb;border-radius:6px;padding:2px 7px;font-family:ui-monospace,Consolas,monospace}
.tiles{display:grid;grid-template-columns:repeat(3,1fr);gap:14px;margin-bottom:20px}
.tile{background:var(--panel);border:1px solid var(--line);border-radius:var(--radius);padding:16px 18px}
.tile .lb{color:var(--muted);font-size:12px}.tile b{display:block;font-size:28px;font-weight:700;margin-top:5px;letter-spacing:-.5px}
.tile.warn b{color:var(--warn)}.tile.ok b{color:var(--ok)}.tile .ft{color:var(--muted);font-size:11px;margin-top:5px}
h2{font-size:14px;margin:22px 0 9px;font-weight:650}
.card{background:var(--panel);border:1px solid var(--line);border-radius:var(--radius);overflow:hidden}
table{width:100%;border-collapse:collapse;font-size:12.5px}
th{text-align:left;color:var(--muted);font-weight:500;font-size:11px;padding:10px 14px;border-bottom:1px solid var(--line);background:#fafbfd}
td{padding:10px 14px;border-bottom:1px solid var(--line);vertical-align:top}tr:last-child td{border-bottom:0}
code{font-family:ui-monospace,Consolas,monospace;font-size:11.5px}
.tag{font-size:11px;padding:2px 9px;border-radius:999px;white-space:nowrap}
.tag.o200k,.tag.cl100k{background:var(--ok-soft);color:var(--ok)}
.tag.suspected_non_gpt,.tag.tokenizer_fingerprint{background:var(--danger-soft);color:var(--danger)}
.tag.insufficient,.tag.ambiguous,.tag.undetermined{background:#eef1f5;color:var(--muted)}
.tag.self_reported{background:var(--warn-soft);color:var(--warn)}
.danger{color:var(--danger);font-weight:600}
.empty{text-align:center;color:var(--muted);padding:26px}
.limits{color:var(--muted);font-size:11px;line-height:1.9;margin-top:18px}
.toast{position:fixed;right:20px;bottom:20px;background:#0f1e2e;color:#fff;padding:11px 16px;border-radius:10px;font-size:12.5px;opacity:0;transition:opacity .2s;pointer-events:none}
.toast.show{opacity:1}
</style></head>
<body>
<aside class="side">
 <div class="brand"><span class="mk">保</span><span>Codex 保安<small>MODEL AUDIT</small></span></div>
 <button class="nav on">📊 模型审计日报</button>
 <div class="foot"><span class="dot" id="livedot"></span><span id="livetext">本机运行中</span><p>数据留在本机 · 北京时间</p></div>
</aside>
<div class="main">
 <div class="top">
  <div><h1>模型审计日报</h1><div class="sub">读一天的日志：总请求 / 异常 / 异常请求实际是什么模型（家族级，非权重认证）</div></div>
  <span class="spacer"></span>
  <span class="pill bad" id="certpill">证书未信任</span>
  <button class="btn" id="certbtn">一键信任证书</button>
  <input type="date" id="date">
  <button class="btn ghost" id="refresh">刷新</button>
  <button class="btn ghost" id="theme" title="切换主题">🌓</button>
 </div>
 <div class="body">
  <div class="setup" id="setup">
   <span class="step">① 证书 <b id="s-cert">检查中…</b></span>
   <span class="step">② 接入 <b id="s-attach">检查中…</b> <code id="s-proxy">—</code></span>
   <button class="btn" id="attachbtn" style="display:none">一键接入 CC Switch</button>
   <button class="btn ghost" id="detachbtn" style="display:none">断开</button>
   <span class="step" id="s-observe"></span>
  </div>
  <div class="tiles">
   <div class="tile"><span class="lb">总请求</span><b id="total">—</b><div class="ft" id="total-ft">经本机观察的上游请求</div></div>
   <div class="tile warn"><span class="lb">异常请求</span><b id="anom">—</b><div class="ft">路由替换 / Token 矛盾</div></div>
   <div class="tile ok"><span class="lb">正常请求</span><b id="clean">—</b><div class="ft">未命中异常</div></div>
  </div>
  <h2>异常请求实际是什么模型</h2>
  <div class="card"><table><thead><tr><th>请求模型</th><th>实际模型</th><th>证据级别</th><th>次数</th></tr></thead><tbody id="actual"></tbody></table></div>
  <h2>分词器指纹（按请求模型分组）</h2>
  <div class="card"><table><thead><tr><th>请求模型</th><th>样本</th><th>上游自报</th><th>分词器家族</th><th>slope</th><th>说明</th></tr></thead><tbody id="groups"></tbody></table></div>
  <div class="limits" id="limits"></div>
 </div>
</div>
<div class="toast" id="toast"></div>
<script>
const $=id=>document.getElementById(id);
const ev={self_reported:"中转自报",tokenizer_fingerprint:"分词器指纹",undetermined:"无法判定"};
$("date").value=new Intl.DateTimeFormat("en-CA",{timeZone:"Asia/Shanghai"}).format(new Date());
function toast(t){const el=$("toast");el.textContent=t;el.classList.add("show");setTimeout(()=>el.classList.remove("show"),2600);}
async function status(){
 try{const s=await(await fetch("/api/status")).json();
  $("s-proxy").textContent=s.proxy_url;
  const ok=s.cert_trusted;
  $("certpill").className="pill "+(ok?"ok":"bad");$("certpill").textContent=ok?"证书已信任":"证书未信任";
  $("s-cert").textContent=ok?"已信任 ✓":"未信任";
  $("certbtn").style.display=ok?"none":"inline-block";
  const rt=s.routing||{};
  if(!rt.ccswitch_found){$("s-attach").textContent="未找到 CC Switch";$("attachbtn").style.display="none";$("detachbtn").style.display="none";}
  else if(rt.attached){$("s-attach").textContent="已接入 ✓";$("attachbtn").style.display="none";$("detachbtn").style.display="inline-block";}
  else{$("s-attach").textContent="未接入";$("attachbtn").style.display="inline-block";$("detachbtn").style.display="none";}
  const seen=rt.last_seen_sec;
  $("s-observe").innerHTML = seen>=0&&seen<600 ? "· <b style='color:var(--ok)'>观察中</b>（"+seen+"s 前有流量）" : (rt.attached?"· 等待新会话流量…":"");
  $("livetext").textContent = (seen>=0&&seen<600)?"观察中":"本机运行中";
 }catch(e){}
}
async function post(url,okmsg){try{const r=await(await fetch(url,{method:"POST"})).json();toast(r.ok?(r.message||okmsg):("失败："+(r.error||"")));}catch(e){toast("操作失败");}status();}

$("certbtn").onclick=async()=>{$("certbtn").disabled=true;$("certbtn").textContent="安装中…";
 try{const r=await(await fetch("/api/cert/install",{method:"POST"})).json();
  toast(r.ok?"证书已装入当前用户信任库":("安装失败："+(r.error||"")));}catch(e){toast("安装失败");}
 $("certbtn").disabled=false;$("certbtn").textContent="一键信任证书";status();};
$("attachbtn").onclick=()=>post("/api/attach","已接入");
$("detachbtn").onclick=()=>post("/api/detach","已断开");
async function load(){
 const rep=await(await fetch("/api/report?date="+$("date").value)).json();
 $("total").textContent=rep.total_requests;$("anom").textContent=rep.routing_substitutions+rep.token_anomalies;
 $("clean").textContent=Math.max(0,rep.total_requests-rep.routing_substitutions-rep.token_anomalies);
 const a=$("actual");a.innerHTML="";
 (rep.actual_models||[]).forEach(m=>{a.insertAdjacentHTML("beforeend",
  "<tr><td><code>"+m.requested_model+"</code></td><td><code class=danger>"+m.actual_model+"</code></td><td><span class='tag "+m.evidence+"'>"+(ev[m.evidence]||m.evidence)+"</span></td><td>"+m.count+"</td></tr>");});
 if(!(rep.actual_models||[]).length)a.innerHTML="<tr><td colspan=4 class=empty>当日未检测到异常请求。</td></tr>";
 const g=$("groups");g.innerHTML="";
 (rep.groups||[]).forEach(x=>{g.insertAdjacentHTML("beforeend",
  "<tr><td><code>"+x.requested_model+"</code></td><td>"+x.samples+"</td><td><code>"+((x.reported_models||[]).join(", ")||"—")+"</code></td><td><span class='tag "+x.family+"'>"+x.family_label+"</span></td><td>"+(x.slope||"—")+"</td><td style='color:var(--muted)'>"+(x.note||"")+"</td></tr>");});
 if(!(rep.groups||[]).length)g.innerHTML="<tr><td colspan=6 class=empty>暂无可指纹样本（需经本机观察到带正文的请求）。</td></tr>";
 $("limits").innerHTML="说明：<br>"+(rep.limitations||[]).map(l=>"· "+l).join("<br>");
}
$("refresh").onclick=()=>{status();load();};$("date").onchange=load;
function applyTheme(t){document.body.dataset.theme=t;localStorage.setItem("theme",t);}
$("theme").onclick=()=>applyTheme(document.body.dataset.theme==="dark"?"light":"dark");
applyTheme(localStorage.getItem("theme")||"light");
status();load();setInterval(()=>{status();load();},10000);
</script>
</body></html>`
