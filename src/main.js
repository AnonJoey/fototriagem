import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import { open as openDialog } from '@tauri-apps/plugin-dialog';

// App State
const state = {
  view: 'setup', // 'setup' | 'months' | 'triage'
  origem: '',
  destino: '',
  incluirEspeciais: false,
  scanData: null,
  meses: [],
  selectedMonthIndex: 0,
  currentMonth: null,
  fila: [],
  pos: 0,
  undoCount: 0,
  folders: [],
  modalMode: 'list', // 'list' | 'input_create' | 'choose_rename' | 'input_rename'
  selectedFolderIndex: 0,
  targetRenameFolder: null,
  explorerPath: null,
  explorerData: null,
  selectedExplorerIndex: 0,
};

// Check if running inside Tauri
const isTauri = window.__TAURI_INTERNALS__ !== undefined;

// DOM Elements
const views = {
  setup: document.getElementById('view-setup'),
  months: document.getElementById('view-months'),
  explorer: document.getElementById('view-explorer'),
  triage: document.getElementById('view-triage'),
};

const setupEls = {
  inputOrigem: document.getElementById('input-origem'),
  btnBrowseOrigem: document.getElementById('btn-browse-origem'),
  inputDestino: document.getElementById('input-destino'),
  btnBrowseDestino: document.getElementById('btn-browse-destino'),
  chkEspeciais: document.getElementById('chk-especiais'),
  btnIniciar: document.getElementById('btn-iniciar-scan'),
  btnAbrirExplorador: document.getElementById('btn-abrir-explorador'),
};

const monthsEls = {
  container: document.getElementById('months-list-container'),
  summary: document.getElementById('months-summary-label'),
  btnBack: document.getElementById('btn-voltar-setup'),
  btnUndo: document.getElementById('btn-months-undo'),
};

const explorerEls = {
  currentPath: document.getElementById('explorer-current-path'),
  btnUp: document.getElementById('btn-explorer-up'),
  btnNewFolder: document.getElementById('btn-explorer-new-folder'),
  btnBack: document.getElementById('btn-explorer-back'),
  statsPhotos: document.getElementById('explorer-stats-photos'),
  statsVideos: document.getElementById('explorer-stats-videos'),
  statsSize: document.getElementById('explorer-stats-size'),
  btnSetOrigem: document.getElementById('btn-explorer-set-origem'),
  btnSetDestino: document.getElementById('btn-explorer-set-destino'),
  folderList: document.getElementById('explorer-folder-list'),
};

const triageEls = {
  monthTitle: document.getElementById('triage-month-title'),
  pos: document.getElementById('triage-pos'),
  date: document.getElementById('triage-date'),
  tag: document.getElementById('triage-tag'),
  copies: document.getElementById('triage-copies'),
  cardCenter: document.getElementById('card-center'),
  activeImg: document.getElementById('active-img'),
  activeVideo: document.getElementById('active-video'),
  activeEmpty: document.getElementById('active-empty'),
  cardPrev: document.getElementById('card-prev'),
  cardNext: document.getElementById('card-next'),
  btnEmptyReturn: document.getElementById('btn-empty-return'),
  btnEmptyUndo: document.getElementById('btn-empty-undo'),
  btnUndo: document.getElementById('btn-action-undo'),
  undoCount: document.getElementById('undo-count'),
  btnFolders: document.getElementById('btn-action-folders'),
  btnMenu: document.getElementById('btn-action-menu'),
  keyUp: document.getElementById('key-up'),
  keyDown: document.getElementById('key-down'),
  keyLeft: document.getElementById('key-left'),
  keyRight: document.getElementById('key-right'),
  actionBanner: document.getElementById('triage-action-banner'),
  actionBannerText: document.getElementById('action-banner-text'),
  btnBannerUndo: document.getElementById('btn-banner-undo'),
};

const modalEls = {
  overlay: document.getElementById('modal-folders'),
  title: document.getElementById('modal-folder-title'),
  list: document.getElementById('modal-folder-list'),
  inputBox: document.getElementById('modal-input-container'),
  inputLabel: document.getElementById('modal-input-label'),
  inputName: document.getElementById('input-folder-name'),
  hintLeft: document.getElementById('modal-hint-left'),
  hintRight: document.getElementById('modal-hint-right'),
};

// ==========================================================================
// DIAGNOSTIC LOGGING & ERROR MONITORING
// ==========================================================================
const diagEls = {
  badge: document.getElementById('diag-badge'),
  dot: document.getElementById('diag-status-dot'),
  summary: document.getElementById('diag-summary-text'),
  drawer: document.getElementById('diag-drawer'),
  container: document.getElementById('diag-log-container'),
  btnCopy: document.getElementById('btn-diag-copy'),
  btnClear: document.getElementById('btn-diag-clear'),
  btnClose: document.getElementById('btn-diag-close'),
};

const diagState = {
  logs: [],
  errors: 0,
  warnings: 0,
};

function diagLog(level, message, details = null) {
  const time = new Date().toLocaleTimeString('pt-BR');
  const entry = { time, level, message, details };
  diagState.logs.push(entry);

  if (level === 'ERROR') {
    diagState.errors++;
    diagEls.dot.style.background = '#ef4444';
    diagEls.drawer.style.display = 'flex';
  } else if (level === 'WARN') {
    diagState.warnings++;
    if (diagState.errors === 0) diagEls.dot.style.background = '#f59e0b';
  }

  diagEls.summary.textContent = `DIAGNÓSTICOS (${diagState.errors} erros, ${diagState.logs.length} logs)`;

  const el = document.createElement('div');
  el.style.borderBottom = '1px solid #1e1e28';
  el.style.paddingBottom = '4px';

  let color = '#a1a1aa';
  let badge = `[${level}]`;
  if (level === 'ERROR') { color = '#f87171'; badge = '❌ [ERRO]'; }
  else if (level === 'WARN') { color = '#fbbf24'; badge = '⚠️ [AVISO]'; }
  else if (level === 'IPC') { color = '#60a5fa'; badge = '⚡ [IPC]'; }
  else if (level === 'SUCCESS') { color = '#34d399'; badge = '✓ [OK]'; }

  el.innerHTML = `<span style="color:#64748b;">${time}</span> <b style="color:${color};">${badge}</b> <span>${message}</span>`;
  if (details) {
    const detEl = document.createElement('pre');
    detEl.style.fontSize = '10px';
    detEl.style.color = '#94a3b8';
    detEl.style.margin = '2px 0 0 0';
    detEl.style.whiteSpace = 'pre-wrap';
    detEl.textContent = typeof details === 'object' ? JSON.stringify(details, null, 2) : String(details);
    el.appendChild(detEl);
  }

  if (diagState.logs.length === 1) diagEls.container.innerHTML = '';
  diagEls.container.appendChild(el);
  diagEls.container.scrollTop = diagEls.container.scrollHeight;

  console.log(`[DIAG:${level}]`, message, details || '');
}

// Global Uncaught Exception Listeners
window.addEventListener('error', (e) => {
  diagLog('ERROR', `Erro JS não capturado: ${e.message}`, {
    filename: e.filename,
    lineno: e.lineno,
    colno: e.colno,
    stack: e.error ? e.error.stack : null,
  });
});

window.addEventListener('unhandledrejection', (e) => {
  diagLog('ERROR', `Promise Rejeitada: ${e.reason}`, {
    reason: e.reason ? (e.reason.stack || e.reason.message || e.reason) : 'Desconhecido',
  });
});

// Diagnostics Drawer Controls
diagEls.badge.addEventListener('click', () => {
  const isHidden = diagEls.drawer.style.display === 'none' || !diagEls.drawer.style.display;
  diagEls.drawer.style.display = isHidden ? 'flex' : 'none';
});

diagEls.btnClose.addEventListener('click', () => {
  diagEls.drawer.style.display = 'none';
});

diagEls.btnClear.addEventListener('click', () => {
  diagState.logs = [];
  diagState.errors = 0;
  diagState.warnings = 0;
  diagEls.dot.style.background = '#10b981';
  diagEls.summary.textContent = 'DIAGNÓSTICOS (0 erros)';
  diagEls.container.innerHTML = '<div style="color: #71717a;">Logs limpos. Nenhum evento registrado.</div>';
});

diagEls.btnCopy.addEventListener('click', () => {
  const text = diagState.logs.map(l => `[${l.time}] [${l.level}] ${l.message} ${l.details ? JSON.stringify(l.details) : ''}`).join('\n');
  navigator.clipboard.writeText(text);
  showToast('Logs copiados para a área de transferência!');
});

// Safe IPC wrapper
async function safeInvoke(cmd, args = {}) {
  const t0 = performance.now();
  diagLog('IPC', `Invocando comando: ${cmd}`, args);
  try {
    const res = await invoke(cmd, args);
    const dt = (performance.now() - t0).toFixed(1);
    diagLog('SUCCESS', `${cmd} concluído com sucesso (${dt}ms)`);
    return res;
  } catch (err) {
    const dt = (performance.now() - t0).toFixed(1);
    diagLog('ERROR', `Falha no comando ${cmd} após ${dt}ms: ${err}`, { cmd, args, error: err });
    throw err;
  }
}

// Image Base64 Cache
const mediaCache = new Map();

async function getMediaDataUrl(path) {
  if (!path) return '';
  if (mediaCache.has(path)) {
    return mediaCache.get(path);
  }
  if (isTauri) {
    try {
      const dataUrl = await safeInvoke('load_image_data', { path });
      mediaCache.set(path, dataUrl);
      return dataUrl;
    } catch (err) {
      diagLog('WARN', `load_image_data falhou para ${path}, tentando convertFileSrc...`, err);
      const fallback = convertFileSrc(path);
      mediaCache.set(path, fallback);
      return fallback;
    }
  }
  return path;
}

// Pre-fill paths if available
(async () => {
  diagLog('INFO', 'Inicializando GS Photo Triage...', {
    platform: navigator.platform,
    userAgent: navigator.userAgent,
    isTauri,
  });

  if (isTauri) {
    try {
      const paths = await safeInvoke('get_default_paths');
      if (paths && paths.length >= 2) {
        setupEls.inputOrigem.value = paths[0];
        setupEls.inputDestino.value = paths[1];
        state.origem = paths[0];
        state.destino = paths[1];
        diagLog('INFO', `Caminhos de teste carregados: ${paths[0]}`);
      }
    } catch (e) {
      diagLog('WARN', 'Não foi possível obter caminhos padrão:', e);
    }
  } else {
    setupEls.inputOrigem.value = '/home/joey/Pictures/Teste_Fototriagem/Google Photos';
    setupEls.inputDestino.value = '/home/joey/Pictures/Teste_Fototriagem/Destino_Organizado';
  }
})();

// ==========================================================================
// TOAST NOTIFICATIONS
// ==========================================================================
function showToast(msg) {
  const container = document.getElementById('toast-container');
  const toast = document.createElement('div');
  toast.className = 'toast';
  toast.textContent = msg;
  container.appendChild(toast);
  setTimeout(() => toast.remove(), 3200);
}

// ==========================================================================
// VIEW ROUTING
// ==========================================================================
function switchView(viewName) {
  state.view = viewName;
  Object.keys(views).forEach((k) => {
    views[k].classList.toggle('active', k === viewName);
  });

  if (viewName === 'months') {
    renderMonthsList();
  } else if (viewName === 'triage') {
    renderTriageView();
  } else if (viewName === 'explorer') {
    loadExplorer(state.explorerPath || state.origem || '/home/joey/Pictures');
  }
}

// ==========================================================================
// SETUP SCREEN LOGIC
// ==========================================================================
setupEls.btnBrowseOrigem.addEventListener('click', async () => {
  if (isTauri) {
    try {
      const selected = await openDialog({ directory: true, multiple: false, title: 'Selecione a Pasta de Origem' });
      if (selected) {
        setupEls.inputOrigem.value = selected;
        state.origem = selected;
      }
    } catch {
      const p = prompt('Digite o caminho da pasta de fotos:', setupEls.inputOrigem.value);
      if (p) {
        setupEls.inputOrigem.value = p;
        state.origem = p;
      }
    }
  } else {
    const p = prompt('Digite o caminho da pasta de fotos:', '/home/joey/Pictures/Teste_Fototriagem/Google Photos');
    if (p) setupEls.inputOrigem.value = p;
  }
});

setupEls.btnBrowseDestino.addEventListener('click', async () => {
  if (isTauri) {
    try {
      const selected = await openDialog({ directory: true, multiple: false, title: 'Selecione a Pasta de Destino' });
      if (selected) {
        setupEls.inputDestino.value = selected;
        state.destino = selected;
      }
    } catch {
      const p = prompt('Digite o caminho da pasta de destino:', setupEls.inputDestino.value);
      if (p) {
        setupEls.inputDestino.value = p;
        state.destino = p;
      }
    }
  } else {
    const p = prompt('Digite o caminho da pasta de destino:', '/home/joey/Pictures/Teste_Fototriagem/Destino_Organizado');
    if (p) setupEls.inputDestino.value = p;
  }
});

setupEls.btnAbrirExplorador.addEventListener('click', () => {
  switchView('explorer');
});

setupEls.btnIniciar.addEventListener('click', startScan);

// ==========================================================================
// SYSTEM FOLDER EXPLORER LOGIC
// ==========================================================================
async function loadExplorer(path) {
  try {
    if (isTauri) {
      const data = await safeInvoke('list_system_folders', { path: path || null });
      state.explorerData = data;
      state.explorerPath = data.caminho_atual;
      state.selectedExplorerIndex = 0;
      renderExplorerView();
    } else {
      state.explorerPath = path || '/home/joey/Pictures';
      state.explorerData = {
        caminho_atual: state.explorerPath,
        caminho_pai: '/home/joey',
        subpastas: [
          { nome: 'Teste_Fototriagem', caminho: '/home/joey/Pictures/Teste_Fototriagem', total_fotos: 12, total_videos: 2, total_bytes: 4500000, subpastas_count: 3 },
          { nome: 'Wallpapers', caminho: '/home/joey/Pictures/Wallpapers', total_fotos: 45, total_videos: 0, total_bytes: 85000000, subpastas_count: 0 },
          { nome: 'Screenshots', caminho: '/home/joey/Pictures/Screenshots', total_fotos: 8, total_videos: 0, total_bytes: 12000000, subpastas_count: 0 },
        ],
        fotos_diretas: 4,
        videos_diretos: 1,
        bytes_diretos: 15000000,
      };
      state.selectedExplorerIndex = 0;
      renderExplorerView();
    }
  } catch (err) {
    showToast(`Erro ao carregar pastas: ${err}`);
  }
}

function renderExplorerView() {
  if (!state.explorerData) return;
  const d = state.explorerData;
  explorerEls.currentPath.textContent = d.caminho_atual;
  explorerEls.statsPhotos.textContent = `📷 ${d.fotos_diretas} fotos nesta pasta`;
  explorerEls.statsVideos.textContent = `🎥 ${d.videos_diretos} vídeos`;
  const mb = (d.bytes_diretos / (1024 * 1024)).toFixed(1);
  explorerEls.statsSize.textContent = `💾 ${mb} MB`;

  explorerEls.folderList.innerHTML = '';
  if (!d.subpastas || d.subpastas.length === 0) {
    explorerEls.folderList.innerHTML = '<div style="padding: 24px; text-align: center; color: var(--eden-stone);">Nenhuma subpasta encontrada neste diretório.</div>';
    return;
  }

  d.subpastas.forEach((sub, idx) => {
    const card = document.createElement('div');
    card.className = `month-card ${idx === state.selectedExplorerIndex ? 'selected' : ''}`;
    
    const sizeMb = (sub.total_bytes / (1024 * 1024)).toFixed(1);
    const subBadge = sub.subpastas_count > 0 ? `<span class="badge-andamento">${sub.subpastas_count} subpastas</span>` : '';

    card.innerHTML = `
      <div style="display: flex; justify-content: space-between; align-items: center;">
        <div class="month-name" style="font-size: 15px;">📁 ${sub.nome}</div>
        <div style="display: flex; gap: 6px;">
          <button class="btn-browse btn-set-origem-sub" style="padding: 4px 8px; font-size: 10px;" title="Definir como Origem">Usar Origem</button>
          <button class="btn-browse btn-set-destino-sub" style="padding: 4px 8px; font-size: 10px;" title="Definir como Destino">Usar Destino</button>
        </div>
      </div>
      <div class="month-stats-row" style="margin-top: 6px;">
        <span class="badge-concluido">📷 ${sub.total_fotos} fotos</span>
        ${sub.total_videos > 0 ? `<span class="badge-andamento">🎥 ${sub.total_videos} vídeos</span>` : ''}
        ${subBadge}
        <span class="badge-aviso" style="background: rgba(255,255,255,0.06); color: var(--eden-stone); border: none;">${sizeMb} MB</span>
      </div>
    `;

    card.querySelector('.btn-set-origem-sub').addEventListener('click', (e) => {
      e.stopPropagation();
      setupEls.inputOrigem.value = sub.caminho;
      state.origem = sub.caminho;
      showToast(`Origem definida: ${sub.nome}`);
      switchView('setup');
    });

    card.querySelector('.btn-set-destino-sub').addEventListener('click', (e) => {
      e.stopPropagation();
      setupEls.inputDestino.value = sub.caminho;
      state.destino = sub.caminho;
      showToast(`Destino definido: ${sub.nome}`);
      switchView('setup');
    });

    card.addEventListener('click', () => {
      state.selectedExplorerIndex = idx;
      loadExplorer(sub.caminho);
    });

    explorerEls.folderList.appendChild(card);
  });

  const sel = explorerEls.folderList.children[state.selectedExplorerIndex];
  if (sel) sel.scrollIntoView({ block: 'nearest' });
}

explorerEls.btnUp.addEventListener('click', () => {
  if (state.explorerData && state.explorerData.caminho_pai) {
    loadExplorer(state.explorerData.caminho_pai);
  }
});

explorerEls.btnBack.addEventListener('click', () => {
  switchView('setup');
});

explorerEls.btnSetOrigem.addEventListener('click', () => {
  if (state.explorerPath) {
    setupEls.inputOrigem.value = state.explorerPath;
    state.origem = state.explorerPath;
    showToast(`Origem definida: ${state.explorerPath}`);
    switchView('setup');
  }
});

explorerEls.btnSetDestino.addEventListener('click', () => {
  if (state.explorerPath) {
    setupEls.inputDestino.value = state.explorerPath;
    state.destino = state.explorerPath;
    showToast(`Destino definido: ${state.explorerPath}`);
    switchView('setup');
  }
});

explorerEls.btnNewFolder.addEventListener('click', async () => {
  const name = prompt('Nome da nova pasta:');
  if (name && name.trim()) {
    try {
      if (isTauri) {
        await safeInvoke('create_system_folder', {
          parentPath: state.explorerPath,
          folderName: name.trim(),
        });
      }
      showToast(`Pasta "${name}" criada com sucesso!`);
      loadExplorer(state.explorerPath);
    } catch (err) {
      showToast(`Erro ao criar pasta: ${err}`);
    }
  }
});

async function startScan() {
  const origem = setupEls.inputOrigem.value.trim();
  const destino = setupEls.inputDestino.value.trim() || origem;
  const especiais = setupEls.chkEspeciais.checked;

  if (!origem) {
    showToast('Informe a pasta de origem!');
    return;
  }

  setupEls.btnIniciar.disabled = true;
  setupEls.btnIniciar.textContent = 'ESCANEANDO ACERVO...';

  try {
    if (isTauri) {
      const res = await safeInvoke('scan_library', {
        origemPath: origem,
        destinoPath: destino ? destino : null,
        incluirEspeciais: especiais,
      });
      state.scanData = res;
      state.origem = origem;
      state.destino = destino;
      state.meses = res.meses;
      diagLog('INFO', `Varredura concluída: ${res.total_arquivos} arquivos, ${res.duplicatas_removidas} duplicatas`);
      if (res.avisos && res.avisos.length > 0) {
        res.avisos.forEach(av => diagLog('WARN', av));
      }
    } else {
      // Mock data for pure web preview
      state.meses = [
        { ano: 2024, mes: 5, nome_mes: 'Maio', total: 4, restantes: 4, sem_data_real: 0, concluido: false },
        { ano: 2024, mes: 6, nome_mes: 'Junho', total: 12, restantes: 12, sem_data_real: 1, concluido: false },
        { ano: 2024, mes: 7, nome_mes: 'Julho', total: 28, restantes: 0, sem_data_real: 0, concluido: true },
      ];
    }

    state.selectedMonthIndex = 0;
    switchView('months');
  } catch (err) {
    showToast(`Erro ao escanear: ${err}`);
  } finally {
    setupEls.btnIniciar.disabled = false;
    setupEls.btnIniciar.innerHTML = `<span>Iniciar Triagem</span><span style="font-size: 11px; opacity: 0.8;">[ ⏎ ENTER ]</span>`;
  }
}

// ==========================================================================
// MONTHS VIEW LOGIC
// ==========================================================================
monthsEls.btnBack.addEventListener('click', () => switchView('setup'));

function renderMonthsList() {
  monthsEls.container.innerHTML = '';
  if (!state.meses || state.meses.length === 0) {
    monthsEls.container.innerHTML = '<div style="padding: 24px; text-align: center; color: var(--eden-stone);">Nenhum arquivo encontrado.</div>';
    return;
  }

  state.meses.forEach((m, idx) => {
    const card = document.createElement('div');
    card.className = `month-card ${idx === state.selectedMonthIndex ? 'selected' : ''}`;
    
    let badgeHtml = '';
    if (m.concluido) {
      badgeHtml = `<span class="badge-concluido">✓ Concluído</span>`;
    } else if (m.restantes === m.total) {
      badgeHtml = `<span class="badge-andamento">${m.restantes} itens</span>`;
    } else {
      badgeHtml = `<span class="badge-andamento">${m.restantes} restantes</span>`;
    }

    let avisoHtml = m.sem_data_real > 0 ? `<span class="badge-aviso">⚠️ ${m.sem_data_real} s/ data</span>` : '';

    card.innerHTML = `
      <div class="month-name">${m.nome_mes} de ${m.ano}</div>
      <div class="month-stats-row">
        ${avisoHtml}
        ${badgeHtml}
      </div>
    `;

    card.addEventListener('click', () => {
      state.selectedMonthIndex = idx;
      openMonth(m);
    });

    monthsEls.container.appendChild(card);
  });

  const selectedEl = monthsEls.container.children[state.selectedMonthIndex];
  if (selectedEl) selectedEl.scrollIntoView({ block: 'nearest' });
}

async function openMonth(monthData) {
  state.currentMonth = monthData;
  if (isTauri) {
    const items = await safeInvoke('get_month_items', {
      ano: monthData.ano,
      mes: monthData.mes,
    });
    state.fila = items.filter((i) => !i.triada);
    diagLog('INFO', `Mês aberto: ${monthData.nome_mes} ${monthData.ano} (${state.fila.length} itens a triar)`);
  } else {
    state.fila = [
      { caminho: '/home/joey/Pictures/photo1.jpg', nome: 'IMG_20240501_001.jpg', data_formatada: '01/05/2024 14:32', origem_data: 'takeout', copias: [], eh_video: false },
      { caminho: '/home/joey/Pictures/photo2.jpg', nome: 'IMG_20240502_002.jpg', data_formatada: '02/05/2024 18:15', origem_data: 'exif', copias: ['/path/copia.jpg'], eh_video: false },
    ];
  }

  state.pos = 0;
  switchView('triage');
}

// ==========================================================================
// TRIAGE VIEW LOGIC (CAROUSEL & MECHANICAL CONTROLLER)
// ==========================================================================
function getMediaUrl(path) {
  if (!path) return '';
  if (isTauri) {
    return convertFileSrc(path);
  }
  return path;
}

function renderTriageView() {
  if (!state.currentMonth) return;

  triageEls.monthTitle.textContent = `${state.currentMonth.nome_mes} ${state.currentMonth.ano}`;
  triageEls.undoCount.textContent = `(${state.undoCount})`;

  const total = state.fila.length;
  if (total === 0) {
    // Month is finished
    triageEls.pos.textContent = '0 / 0';
    triageEls.date.textContent = 'Concluído';
    triageEls.tag.style.display = 'none';
    triageEls.copies.style.display = 'none';
    triageEls.activeImg.style.display = 'none';
    triageEls.activeVideo.style.display = 'none';
    triageEls.activeEmpty.style.display = 'flex';
    triageEls.cardPrev.innerHTML = '<div class="card-flanking-empty">◄ INÍCIO</div>';
    triageEls.cardNext.innerHTML = '<div class="card-flanking-empty">FIM ►</div>';
    return;
  }

  state.pos = Math.max(0, Math.min(state.pos, total - 1));
  const atual = state.fila[state.pos];

  triageEls.activeEmpty.style.display = 'none';
  triageEls.pos.textContent = `${state.pos + 1} / ${total}`;
  triageEls.date.textContent = atual.data_formatada;
  triageEls.tag.textContent = atual.origem_data.toUpperCase();
  triageEls.tag.style.display = 'inline-block';

  // As cópias idênticas de outros álbuns seguem a decisão para _copias/,
  // então o aviso diz o que vai acontecer, não onde elas estão agora.
  if (atual.copias && atual.copias.length > 0) {
    const n = atual.copias.length;
    triageEls.copies.textContent = `+${n} CÓPIA${n > 1 ? 'S' : ''} → _copias/`;
    triageEls.copies.title = atual.copias
      .map((c) => (typeof c === 'string' ? c : c.caminho))
      .join('\n');
    triageEls.copies.style.display = 'inline-block';
  } else {
    triageEls.copies.style.display = 'none';
  }

  // Active Center Media
  getMediaDataUrl(atual.caminho).then((activeUrl) => {
    if (atual.eh_video) {
      triageEls.activeImg.style.display = 'none';
      triageEls.activeVideo.style.display = 'block';
      triageEls.activeVideo.src = activeUrl;
      triageEls.activeVideo.play().catch(() => {});
    } else {
      triageEls.activeVideo.pause();
      triageEls.activeVideo.style.display = 'none';
      triageEls.activeImg.style.display = 'block';
      triageEls.activeImg.src = activeUrl;
    }
  });

  // Left Flanking Preview
  if (state.pos > 0) {
    const prev = state.fila[state.pos - 1];
    getMediaDataUrl(prev.caminho).then((url) => {
      triageEls.cardPrev.innerHTML = `<img src="${url}" alt="Anterior" />`;
    });
  } else {
    triageEls.cardPrev.innerHTML = '<div class="card-flanking-empty">◄ INÍCIO</div>';
  }

  // Right Flanking Preview
  if (state.pos < total - 1) {
    const next = state.fila[state.pos + 1];
    getMediaDataUrl(next.caminho).then((url) => {
      triageEls.cardNext.innerHTML = `<img src="${url}" alt="Próximo" />`;
    });
  } else {
    triageEls.cardNext.innerHTML = '<div class="card-flanking-empty">FIM ►</div>';
  }
}

// Navigation & Actions
function navigateLeft() {
  if (state.pos > 0) {
    animateKey(triageEls.keyLeft);
    triageEls.cardCenter.classList.add('animate-right');
    setTimeout(() => {
      state.pos--;
      renderTriageView();
      triageEls.cardCenter.classList.remove('animate-right');
    }, 120);
  }
}

function navigateRight() {
  if (state.pos < state.fila.length - 1) {
    animateKey(triageEls.keyRight);
    triageEls.cardCenter.classList.add('animate-left');
    setTimeout(() => {
      state.pos++;
      renderTriageView();
      triageEls.cardCenter.classList.remove('animate-left');
    }, 120);
  }
}

async function actionTrash() {
  if (state.fila.length === 0) return;
  const midia = state.fila[state.pos];
  animateKey(triageEls.keyUp);
  triageEls.cardCenter.classList.add('animate-trash');

  setTimeout(async () => {
    try {
      if (isTauri) {
        await safeInvoke('trash_item', { itemPath: midia.caminho });
      }
      state.undoCount++;
      state.fila.splice(state.pos, 1);
      showToast(`🗑️ ${midia.nome} movido para _lixo`);
      updateActionBanner(`${midia.nome} → _lixo`);
      updateMonthStats();
      renderTriageView();
    } catch (err) {
      showToast(`Erro ao mover para o lixo: ${err}`);
    } finally {
      triageEls.cardCenter.classList.remove('animate-trash');
    }
  }, 140);
}

async function actionUndo() {
  animateKey(triageEls.btnUndo);
  animateKey(monthsEls.btnUndo);
  try {
    if (isTauri) {
      const restored = await safeInvoke('undo_last_action');
      if (!restored) {
        showToast('Nada a desfazer.');
        return;
      }
      state.undoCount = Math.max(0, state.undoCount - 1);
      
      if (state.view === 'triage') {
        if (state.currentMonth && restored.ano === state.currentMonth.ano && restored.mes === state.currentMonth.mes) {
          state.fila.splice(state.pos, 0, restored);
          renderTriageView();
        } else {
          showToast(`✓ Restaurado no acervo: ${restored.nome}`);
        }
      } else if (state.view === 'months') {
        await updateMonthStats();
        renderMonthsList();
      }
      showToast(`✓ Restaurado: ${restored.nome}`);
      updateActionBanner(`✓ Restaurado: ${restored.nome}`);
      await updateMonthStats();
    } else {
      showToast('Desfazer simulado.');
    }
  } catch (err) {
    showToast(`Erro ao desfazer: ${err}`);
  }
}

async function updateMonthStats() {
  if (isTauri) {
    const meses = await safeInvoke('get_months');
    state.meses = meses;
  }
}

function toggleVideoPlayback() {
  if (triageEls.activeVideo.style.display !== 'none') {
    if (triageEls.activeVideo.paused) {
      triageEls.activeVideo.play();
    } else {
      triageEls.activeVideo.pause();
    }
  }
}

function returnToMonths() {
  triageEls.activeVideo.pause();
  updateMonthStats();
  switchView('months');
}

// Click Bindings on Triage Buttons & Keycaps
triageEls.keyUp.addEventListener('click', actionTrash);
triageEls.keyDown.addEventListener('click', openFolderModal);
triageEls.keyLeft.addEventListener('click', navigateLeft);
triageEls.keyRight.addEventListener('click', navigateRight);
triageEls.cardPrev.addEventListener('click', navigateLeft);
triageEls.cardNext.addEventListener('click', navigateRight);
triageEls.btnUndo.addEventListener('click', actionUndo);
triageEls.btnEmptyUndo.addEventListener('click', actionUndo);
monthsEls.btnUndo.addEventListener('click', actionUndo);
triageEls.btnFolders.addEventListener('click', openFolderModal);
triageEls.btnMenu.addEventListener('click', returnToMonths);
triageEls.btnEmptyReturn.addEventListener('click', returnToMonths);

// Persistent Action / Undo Banner Helper
function updateActionBanner(text) {
  if (!triageEls.actionBanner) return;
  if (!text) {
    triageEls.actionBanner.style.display = 'none';
    return;
  }
  if (triageEls.actionBannerText) {
    triageEls.actionBannerText.textContent = text;
  }
  triageEls.actionBanner.style.display = 'flex';
}

if (triageEls.btnBannerUndo) {
  triageEls.btnBannerUndo.addEventListener('click', actionUndo);
}

// Touch & Swipe Gestures on Central Photo Stage
let touchStartX = 0;
let touchStartY = 0;
let touchStartTime = 0;

if (triageEls.cardCenter) {
  triageEls.cardCenter.addEventListener('pointerdown', (e) => {
    touchStartX = e.clientX;
    touchStartY = e.clientY;
    touchStartTime = Date.now();
  });

  triageEls.cardCenter.addEventListener('pointerup', (e) => {
    const dt = Date.now() - touchStartTime;
    if (dt > 1200) return;

    const diffX = e.clientX - touchStartX;
    const diffY = e.clientY - touchStartY;
    const absX = Math.abs(diffX);
    const absY = Math.abs(diffY);

    // Tap on video: toggle play/pause
    if (absX < 15 && absY < 15) {
      if (triageEls.activeVideo && triageEls.activeVideo.style.display !== 'none') {
        toggleVideoPlayback();
      }
      return;
    }

    // Swipe Up -> Trash (> 70px vertical)
    if (diffY < -70 && absY > absX * 1.2) {
      actionTrash();
      return;
    }

    // Swipe Horizontal (> 60px horizontal)
    if (absX > 60 && absX > absY * 1.2) {
      if (diffX < 0) {
        navigateRight();
      } else {
        navigateLeft();
      }
      return;
    }
  });
}

function animateKey(el) {
  if (!el) return;
  el.classList.add('pressed');
  setTimeout(() => el.classList.remove('pressed'), 180);
}

// ==========================================================================
// FOLDER MODAL LOGIC (SINGLE-COLUMN REVERSIBLE FOLDER TRIAGE)
// ==========================================================================
async function openFolderModal() {
  if (state.fila.length === 0) return;
  animateKey(triageEls.keyDown);

  if (isTauri) {
    state.folders = await safeInvoke('get_target_folders');
  } else {
    state.folders = [
      { name: 'Viagem Japao 2024', path: '/dest/Viagem' },
      { name: 'Familia e Amigos', path: '/dest/Familia' },
      { name: 'Documentos', path: '/dest/Documentos' },
    ];
  }

  state.modalMode = 'list';
  state.selectedFolderIndex = 0;
  modalEls.inputBox.style.display = 'none';
  modalEls.list.style.display = 'flex';
  modalEls.title.textContent = 'Mover para Pasta...';
  modalEls.hintLeft.textContent = '↑↓ Navegar opções';
  modalEls.hintRight.textContent = '⏎ Selecionar  ·  ESC Voltar';

  renderFolderList();
  modalEls.overlay.classList.add('active');
}

function closeFolderModal() {
  modalEls.overlay.classList.remove('active');
  modalEls.inputName.value = '';
}

function renderFolderList() {
  modalEls.list.innerHTML = '';

  // Item 0: + CRIAR NOVA PASTA
  const itemNova = document.createElement('div');
  itemNova.className = `modal-item modal-item-action ${state.selectedFolderIndex === 0 ? 'selected' : ''}`;
  itemNova.innerHTML = `<span>+  Criar Nova Pasta</span><span>[⏎]</span>`;
  itemNova.addEventListener('click', () => triggerFolderSelection(0));
  modalEls.list.appendChild(itemNova);

  // Item 1: ~ RENOMEAR UMA PASTA
  const hasFolders = state.folders && state.folders.length > 0;
  if (hasFolders) {
    const itemRenomear = document.createElement('div');
    itemRenomear.className = `modal-item ${state.selectedFolderIndex === 1 ? 'selected' : ''}`;
    itemRenomear.innerHTML = `<span>~  Renomear uma Pasta</span><span>[⏎]</span>`;
    itemRenomear.addEventListener('click', () => triggerFolderSelection(1));
    modalEls.list.appendChild(itemRenomear);
  }

  // Folder Items
  const offset = hasFolders ? 2 : 1;
  state.folders.forEach((f, idx) => {
    const item = document.createElement('div');
    const currentIndex = idx + offset;
    item.className = `modal-item ${state.selectedFolderIndex === currentIndex ? 'selected' : ''}`;
    item.innerHTML = `<span>📁  ${f.name}</span><span style="font-size: 11px; opacity: 0.6;">[⏎ Mover]</span>`;
    item.addEventListener('click', () => triggerFolderSelection(currentIndex));
    modalEls.list.appendChild(item);
  });
}

async function triggerFolderSelection(idx) {
  const hasFolders = state.folders && state.folders.length > 0;
  const offset = hasFolders ? 2 : 1;

  if (idx === 0) {
    // Mode: Create new folder
    state.modalMode = 'input_create';
    modalEls.title.textContent = 'Criar Nova Pasta';
    modalEls.inputLabel.textContent = 'Nome da nova pasta:';
    modalEls.inputName.value = '';
    modalEls.list.style.display = 'none';
    modalEls.inputBox.style.display = 'flex';
    modalEls.inputName.focus();
    modalEls.hintRight.textContent = '⏎ Criar e Mover  ·  ESC Voltar';
  } else if (hasFolders && idx === 1) {
    // Mode: Choose folder to rename
    state.modalMode = 'choose_rename';
    modalEls.title.textContent = 'Escolha a Pasta para Renomear';
    state.selectedFolderIndex = 0;
    renderRenameChoiceList();
  } else {
    // Move to existing folder
    const folder = state.folders[idx - offset];
    if (folder) {
      await executeMoveToFolder(folder.name);
    }
  }
}

function renderRenameChoiceList() {
  modalEls.list.innerHTML = '';
  state.folders.forEach((f, idx) => {
    const item = document.createElement('div');
    item.className = `modal-item ${state.selectedFolderIndex === idx ? 'selected' : ''}`;
    item.innerHTML = `<span>📁  ${f.name}</span><span style="font-size: 11px; opacity: 0.6;">[⏎ Renomear]</span>`;
    item.addEventListener('click', () => {
      state.targetRenameFolder = f.name;
      state.modalMode = 'input_rename';
      modalEls.title.textContent = `Renomear "${f.name}"`;
      modalEls.inputLabel.textContent = 'Novo nome da pasta:';
      modalEls.inputName.value = f.name;
      modalEls.list.style.display = 'none';
      modalEls.inputBox.style.display = 'flex';
      modalEls.inputName.focus();
      modalEls.inputName.select();
    });
    modalEls.list.appendChild(item);
  });
}

async function executeMoveToFolder(folderName) {
  if (state.fila.length === 0) return;
  const midia = state.fila[state.pos];
  closeFolderModal();

  triageEls.cardCenter.classList.add('animate-move');
  setTimeout(async () => {
    try {
      if (isTauri) {
        await safeInvoke('move_item', {
          itemPath: midia.caminho,
          targetFolderName: folderName,
        });
      }
      state.undoCount++;
      state.fila.splice(state.pos, 1);
      showToast(`📁 ${midia.nome} ➔ ${folderName}`);
      updateActionBanner(`${midia.nome} → ${folderName}`);
      updateMonthStats();
      renderTriageView();
    } catch (err) {
      showToast(`Erro ao mover: ${err}`);
    } finally {
      triageEls.cardCenter.classList.remove('animate-move');
    }
  }, 140);
}

async function confirmModalInput() {
  const text = modalEls.inputName.value.trim();
  if (!text) {
    showToast('Informe um nome válido!');
    return;
  }

  if (state.modalMode === 'input_create') {
    try {
      if (isTauri) {
        await safeInvoke('create_target_folder', { name: text });
      }
      await executeMoveToFolder(text);
    } catch (err) {
      showToast(`Erro ao criar pasta: ${err}`);
    }
  } else if (state.modalMode === 'input_rename') {
    try {
      if (isTauri) {
        await safeInvoke('rename_target_folder', {
          oldName: state.targetRenameFolder,
          newName: text,
        });
      }
      showToast(`Pasta renomeada para "${text}"`);
      openFolderModal();
    } catch (err) {
      showToast(`Erro ao renomear: ${err}`);
    }
  }
}

// ==========================================================================
// GLOBAL KEYBOARD SHORTCUTS & NAVIGATION
// ==========================================================================
window.addEventListener('keydown', (e) => {
  // Global Undo with Ctrl+Z
  const isInputFocused = document.activeElement && (document.activeElement.tagName === 'INPUT' || document.activeElement.tagName === 'TEXTAREA');
  if (!isInputFocused && e.key.toLowerCase() === 'z' && (e.ctrlKey || e.metaKey)) {
    e.preventDefault();
    actionUndo();
    return;
  }

  // 1. Modal is Open
  if (modalEls.overlay.classList.contains('active')) {
    if (state.modalMode === 'input_create' || state.modalMode === 'input_rename') {
      if (e.key === 'Enter') {
        e.preventDefault();
        confirmModalInput();
      } else if (e.key === 'Escape') {
        e.preventDefault();
        openFolderModal();
      }
      return;
    }

    if (e.key === 'Escape') {
      e.preventDefault();
      closeFolderModal();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      const max = modalEls.list.children.length;
      if (max > 0) {
        state.selectedFolderIndex = (state.selectedFolderIndex - 1 + max) % max;
        if (state.modalMode === 'choose_rename') renderRenameChoiceList();
        else renderFolderList();
      }
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      const max = modalEls.list.children.length;
      if (max > 0) {
        state.selectedFolderIndex = (state.selectedFolderIndex + 1) % max;
        if (state.modalMode === 'choose_rename') renderRenameChoiceList();
        else renderFolderList();
      }
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (state.modalMode === 'choose_rename') {
        const target = state.folders[state.selectedFolderIndex];
        if (target) {
          state.targetRenameFolder = target.name;
          state.modalMode = 'input_rename';
          modalEls.title.textContent = `Renomear "${target.name}"`;
          modalEls.inputLabel.textContent = 'Novo nome da pasta:';
          modalEls.inputName.value = target.name;
          modalEls.list.style.display = 'none';
          modalEls.inputBox.style.display = 'flex';
          modalEls.inputName.focus();
          modalEls.inputName.select();
        }
      } else {
        triggerFolderSelection(state.selectedFolderIndex);
      }
    }
    return;
  }

  // 2. Setup View
  if (state.view === 'setup') {
    if (e.key === 'Enter' && !setupEls.btnIniciar.disabled && !isInputFocused) {
      e.preventDefault();
      startScan();
    }
    return;
  }

  // 3. Explorer View
  if (state.view === 'explorer') {
    if (e.key === 'Escape') {
      e.preventDefault();
      switchView('setup');
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      const count = state.explorerData && state.explorerData.subpastas ? state.explorerData.subpastas.length : 0;
      if (count > 0) {
        state.selectedExplorerIndex = (state.selectedExplorerIndex - 1 + count) % count;
        renderExplorerView();
      }
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      const count = state.explorerData && state.explorerData.subpastas ? state.explorerData.subpastas.length : 0;
      if (count > 0) {
        state.selectedExplorerIndex = (state.selectedExplorerIndex + 1) % count;
        renderExplorerView();
      }
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (state.explorerData && state.explorerData.subpastas && state.explorerData.subpastas[state.selectedExplorerIndex]) {
        loadExplorer(state.explorerData.subpastas[state.selectedExplorerIndex].caminho);
      }
    } else if (e.key === 'Backspace') {
      e.preventDefault();
      if (state.explorerData && state.explorerData.caminho_pai) {
        loadExplorer(state.explorerData.caminho_pai);
      }
    }
    return;
  }

  // 4. Months View
  if (state.view === 'months') {
    if (e.key === 'Escape') {
      e.preventDefault();
      switchView('setup');
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (state.meses.length > 0) {
        state.selectedMonthIndex = Math.max(0, state.selectedMonthIndex - 1);
        renderMonthsList();
      }
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      if (state.meses.length > 0) {
        state.selectedMonthIndex = Math.min(state.meses.length - 1, state.selectedMonthIndex + 1);
        renderMonthsList();
      }
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (state.meses[state.selectedMonthIndex]) {
        openMonth(state.meses[state.selectedMonthIndex]);
      }
    }
    return;
  }

  // 5. Triage View
  if (state.view === 'triage') {
    if (e.key === 'Escape') {
      e.preventDefault();
      returnToMonths();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      actionTrash();
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      openFolderModal();
    } else if (e.key === 'ArrowLeft') {
      e.preventDefault();
      navigateLeft();
    } else if (e.key === 'ArrowRight') {
      e.preventDefault();
      navigateRight();
    } else if (e.key === ' ' || e.code === 'Space') {
      e.preventDefault();
      toggleVideoPlayback();
    }
  }
});
