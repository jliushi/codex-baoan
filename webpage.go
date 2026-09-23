package main

const indexHTML = `<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Codex 保安 · 模型审计</title>
<style>
:root{
 --bg:#f4f7fb;--panel:#ffffff;--line:#e3eaf2;--ink:#172536;--muted:#718096;
 --blue:#2563eb;--blue-soft:#edf4ff;--ok:#0f9f73;--ok-soft:#e4f7f0;--warn:#b86b0b;--warn-soft:#fff4df;--danger:#d23f50;--danger-soft:#fff0f1;
 --shadow:0 12px 32px rgba(29,55,88,.07);--radius:16px;font-family:"Segoe UI","Microsoft YaHei",system-ui,sans-serif}
body[data-theme=dark]{--bg:#0f1720;--panel:#17212e;--line:#26333f;--ink:#e6edf3;--muted:#8b9bab;
 --blue-soft:#162c52;--ok-soft:#123b32;--warn-soft:#3b2b0f;--danger-soft:#3c1d25;--shadow:0 12px 32px rgba(0,0,0,.18)}
body[data-theme=dark] th{background:#1c2735}
body[data-theme=dark] .top{background:rgba(23,33,46,.88)}
body[data-theme=dark] .hero{border-color:#2e4664;background:linear-gradient(115deg,#172d50 0%,#1a293d 54%,#173b36 100%)}
body[data-theme=dark] .hero p{color:#a6b6c8}.hero-badge{color:#2c5b9f}
body[data-theme=dark] .hero-badge{background:#1b2b3d;border-color:#355171;color:#b9d3f6}
*{box-sizing:border-box}
body{margin:0;background:radial-gradient(circle at 78% -20%,#e5efff 0,transparent 38%),var(--bg);color:var(--ink);font-size:14px;display:flex;min-height:100vh}
.side{width:238px;flex:0 0 238px;background:linear-gradient(180deg,#102746 0%,#0c1c30 100%);color:#c2d0dc;display:flex;flex-direction:column;padding:28px 18px}
.brand{display:flex;align-items:center;gap:12px;font-size:16px;font-weight:700;color:#fff;margin:0 8px 38px}
.brand .mk{width:38px;height:38px;border-radius:12px;background:linear-gradient(135deg,#5b9bff,#27c3a1);display:grid;place-items:center;font-size:19px;box-shadow:0 8px 20px rgba(39,195,161,.2)}
.brand small{display:block;font-size:9px;letter-spacing:2.2px;color:#8fa8c0;font-weight:600;margin-top:4px}
.nav{border:1px solid transparent;background:none;color:#9fb2c5;text-align:left;padding:12px 13px;border-radius:11px;font:inherit;cursor:pointer;display:flex;gap:10px;align-items:center;margin-bottom:6px}
.nav.on,.nav:hover{background:rgba(105,160,240,.15);border-color:rgba(147,191,255,.12);color:#f3f8ff}
.side .foot{margin:auto 8px 2px;font-size:11px;color:#89a0b7;line-height:1.9}
.dot{display:inline-block;width:8px;height:8px;border-radius:50%;background:#3fbf83;margin-right:6px}.dot.off{background:#c9954a}
.main{flex:1;min-width:0;display:flex;flex-direction:column}
.top{display:flex;align-items:center;gap:14px;padding:20px 34px;border-bottom:1px solid var(--line);background:rgba(255,255,255,.86);backdrop-filter:blur(10px)}
.top h1{font-size:18px;margin:0;font-weight:700;letter-spacing:-.2px}.top .sub{color:var(--muted);font-size:12px;margin-top:4px}
.spacer{flex:1}
.pill{display:inline-flex;align-items:center;gap:7px;font-size:12px;padding:6px 11px;border-radius:999px;font-weight:600}
.pill.ok{background:var(--ok-soft);color:var(--ok)}.pill.bad{background:var(--warn-soft);color:var(--warn)}
input[type=date]{background:var(--panel);border:1px solid var(--line);color:var(--ink);border-radius:9px;padding:7px 10px;font:inherit}
.btn{background:linear-gradient(135deg,#2f73ef,#235bd2);border:0;color:#fff;border-radius:10px;padding:9px 16px;font:inherit;font-weight:650;cursor:pointer;box-shadow:0 5px 12px rgba(37,99,235,.18)}
.btn:hover{filter:brightness(1.06)}.btn.ghost{background:var(--panel);border:1px solid var(--line);color:#33475b;font-weight:500}
.btn:disabled{opacity:.5;cursor:default}
.body{padding:28px 34px 40px;overflow:auto;max-width:1480px;width:100%}
.hero{display:flex;justify-content:space-between;align-items:flex-end;gap:20px;margin-bottom:22px;padding:24px 26px;border:1px solid #d9e6fa;border-radius:18px;background:linear-gradient(115deg,#eaf2ff 0%,#f8fbff 54%,#e7f8f3 100%);box-shadow:var(--shadow)}
.hero .eyebrow{font-size:10px;font-weight:750;letter-spacing:2px;color:#3870c9}.hero h2{font-size:24px;margin:7px 0 5px;letter-spacing:-.7px}.hero p{margin:0;color:#5e7187;font-size:13px;line-height:1.65}.hero-badge{white-space:nowrap;background:#fff;border:1px solid #d7e5f6;border-radius:999px;color:#2c5b9f;padding:9px 13px;font-size:12px;font-weight:650}
.setup{background:var(--panel);border:1px solid var(--line);border-radius:var(--radius);padding:15px 17px;margin-bottom:20px;display:flex;gap:18px;align-items:center;flex-wrap:wrap;box-shadow:var(--shadow)}
.setup .step{font-size:12.5px;color:#2b4a74}.setup code{background:#fff;border:1px solid #d3e2fb;border-radius:6px;padding:2px 7px;font-family:ui-monospace,Consolas,monospace}
.tiles{display:grid;grid-template-columns:repeat(3,1fr);gap:16px;margin-bottom:24px}
.tile{background:var(--panel);border:1px solid var(--line);border-radius:var(--radius);padding:19px 20px;box-shadow:var(--shadow);position:relative;overflow:hidden}.tile:after{content:"";position:absolute;right:-18px;top:-18px;width:70px;height:70px;border-radius:50%;background:var(--blue-soft)}
.tile .lb{color:var(--muted);font-size:12px}.tile b{display:block;font-size:28px;font-weight:700;margin-top:5px;letter-spacing:-.5px}
.tile.warn b{color:var(--warn)}.tile.ok b{color:var(--ok)}.tile .ft{color:var(--muted);font-size:11px;margin-top:5px}
h2{font-size:14px;margin:24px 0 10px;font-weight:700}.card{background:var(--panel);border:1px solid var(--line);border-radius:var(--radius);overflow:hidden;box-shadow:var(--shadow)}
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
@media(max-width:860px){.side{width:74px;flex-basis:74px;padding:22px 10px}.brand{margin:0 auto 34px}.brand span:last-child,.nav span:last-child,.side .foot p{display:none}.brand .mk{width:42px;height:42px}.nav{justify-content:center;padding:12px 8px}.top,.body{padding-left:20px;padding-right:20px}.hero{align-items:flex-start;flex-direction:column}.hero-badge{white-space:normal}}
@media(max-width:620px){.top{flex-wrap:wrap}.top .spacer{display:none}.body{padding-top:20px}.tiles{grid-template-columns:1fr}.setup{gap:10px}.hero h2{font-size:21px}}
</style></head>
<body>
<aside class="side">
 <div class="brand"><span class="mk">保</span><span>Codex 保安<small>MODEL AUDIT</small></span></div>
 <button class="nav on"><span>📊</span><span>模型审计日报</span></button>
 <div class="foot"><span class="dot" id="livedot"></span><span id="livetext">本机运行中</span><p>数据留在本机 · 北京时间</p></div>
</aside>
<div class="main">
 <div class="top">
   <div><h1>模型审计日报</h1><div class="sub">本机采集 · 家族级指纹 · 不上传请求内容</div></div>
  <span class="spacer"></span>
  <span class="pill bad" id="certpill">证书未信任</span>
  <button class="btn" id="certbtn">一键信任证书</button>
  <input type="date" id="date">
  <button class="btn ghost" id="refresh">刷新</button>
  <button class="btn ghost" id="theme" title="切换主题">🌓</button>
 </div>
  <div class="body">
   <section class="hero"><div><div class="eyebrow">LOCAL MODEL AUDIT</div><h2>看清每一次上游请求</h2><p>捕获请求模型、上游自报与 token 指纹，帮助你发现静默替换。接入是临时的，关闭本工具会自动恢复 CC Switch 原出口。</p></div><div class="hero-badge">🔒 数据只留在本机</div></section>
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
status();load();
// 事件驱动：有新请求被观察到才刷新，不做固定轮询（省资源）。
try{const es=new EventSource("/api/events");es.onmessage=()=>{if(!document.hidden)load();};}catch(e){}
document.addEventListener("visibilitychange",()=>{if(!document.hidden){status();load();}});
// 证书/接入状态变化很少：仅在可见时每 30 秒轻量刷新一次。
setInterval(()=>{if(!document.hidden)status();},30000);
</script>
</body></html>`
