import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import { open as openDialog } from '@tauri-apps/plugin-dialog';

// App State
const state = {
  view: 'setup', // 'setup' | 'months' | 'triage'
  origem: '',
  destino: '',
  incluirEspeciais: false,
  mode: 'local', // 'local' | 'immich'
  immichUrl: localStorage.getItem('immich_url') || 'http://localhost:2283',
  immichApiKey: localStorage.getItem('immich_key') || 'WFDC1OGoRioMAGCvxtykUnGyk7yeGIzK7PwpIrknqdc',
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
  isActionBusy: false,
  imageLoadSeq: 0,
};

// Check if running inside Tauri
const isTauri = window.__TAURI_INTERNALS__ !== undefined;

function escapeHtml(str) {
  if (typeof str !== 'string') return '';
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

// DOM Elements
const views = {
  setup: document.getElementById('view-setup'),
  months: document.getElementById('view-months'),
  explorer: document.getElementById('view-explorer'),
  triage: document.getElementById('view-triage'),
};

const setupEls = {
  tabLocal: document.getElementById('tab-mode-local'),
  tabImmich: document.getElementById('tab-mode-immich'),
  modeLocal: document.getElementById('setup-mode-local'),
  modeImmich: document.getElementById('setup-mode-immich'),
  inputImmichUrl: document.getElementById('input-immich-url'),
  inputImmichKey: document.getElementById('input-immich-key'),
  btnImmichTest: document.getElementById('btn-immich-test'),
  btnImmichConectar: document.getElementById('btn-immich-conectar'),
  immichStatusText: document.getElementById('immich-status-text'),
  inputOrigem: document.getElementById('input-origem'),
  btnBrowseOrigem: document.getElementById('btn-browse-origem'),
  inputDestino: document.getElementById('input-destino'),
  btnBrowseDestino: document.getElementById('btn-browse-destino'),
  chkEspeciais: document.getElementById('chk-especiais'),
  btnIniciar: document.getElementById('btn-iniciar-scan'),
  btnAbrirExplorador: document.getElementById('btn-abrir-explorador'),
  sugestoesBox: document.getElementById('setup-sugestoes-box'),
  sugestoesLista: document.getElementById('setup-sugestoes-lista'),
  resumoValidacao: document.getElementById('setup-resumo-validacao'),
  origemPreview: document.getElementById('setup-origem-preview'),
  destinoPreview: document.getElementById('setup-destino-preview'),
  statusAlerta: document.getElementById('setup-status-alerta'),
  modalConfirmScan: document.getElementById('modal-confirm-scan'),
  modalConfirmOrigemText: document.getElementById('modal-confirm-origem-text'),
  modalConfirmDestinoText: document.getElementById('modal-confirm-destino-text'),
  btnConfirmScanExecutar: document.getElementById('btn-confirm-scan-executar'),
  btnConfirmScanCancelar: document.getElementById('btn-confirm-scan-cancelar'),
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

  if (diagState.logs.length > 100) {
    diagState.logs.shift();
  }

  if (level === 'ERROR') {
    diagState.errors++;
    diagEls.dot.style.background = 'var(--gs-error)';
    diagEls.drawer.style.display = 'flex';
  } else if (level === 'WARN') {
    diagState.warnings++;
    if (diagState.errors === 0) diagEls.dot.style.background = 'var(--gs-warn)';
  }

  diagEls.summary.textContent = `DIAGNÓSTICOS (${diagState.errors} erros, ${diagState.logs.length} logs)`;

  const el = document.createElement('div');
  el.style.borderBottom = '1px solid #1e1e28';
  el.style.paddingBottom = '4px';

  let color = 'var(--gs-text-secondary)';
  let badge = `[${level}]`;
  if (level === 'ERROR') { color = 'var(--gs-error)'; badge = '❌ [ERRO]'; }
  else if (level === 'WARN') { color = 'var(--gs-warn)'; badge = '⚠️ [AVISO]'; }
  else if (level === 'IPC') { color = 'var(--gs-text-secondary)'; badge = '⚡ [IPC]'; }
  else if (level === 'SUCCESS') { color = 'var(--gs-ok)'; badge = '✓ [OK]'; }

  el.innerHTML = `<span style="color:var(--gs-text-secondary);">${time}</span> <b style="color:${color};">${badge}</b> `;
  const msgSpan = document.createElement('span');
  msgSpan.textContent = String(message);
  el.appendChild(msgSpan);

  if (details) {
    const detEl = document.createElement('pre');
    detEl.style.fontSize = '10px';
    detEl.style.color = 'var(--gs-text-secondary)';
    detEl.style.margin = '2px 0 0 0';
    detEl.style.whiteSpace = 'pre-wrap';
    detEl.textContent = typeof details === 'object' ? JSON.stringify(details, null, 2) : String(details);
    el.appendChild(detEl);
  }

  while (diagEls.container.children.length >= 100) {
    diagEls.container.removeChild(diagEls.container.firstChild);
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
  diagEls.dot.style.background = 'var(--gs-ok)';
  diagEls.summary.textContent = 'DIAGNÓSTICOS (0 erros)';
  diagEls.container.innerHTML = '<div style="color: var(--gs-text-secondary);">Logs limpos. Nenhum evento registrado.</div>';
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

// Media URL Resolver (zero-copy Tauri asset streaming protocol)
function getMediaUrl(path) {
  if (!path) return '';
  if (isTauri) {
    return convertFileSrc(path);
  }
  return path;
}

// Validacao e seguranca de caminhos
function normalizarCaminho(caminho) {
  if (!caminho || typeof caminho !== 'string') return '';
  let norm = caminho.trim().replace(/\\/g, '/');
  while (norm.length > 1 && norm.endsWith('/')) {
    norm = norm.slice(0, -1);
  }
  return norm;
}

function verificarCaminhos(origem, destino) {
  const o = normalizarCaminho(origem);
  const d = normalizarCaminho(destino);

  if (!o) {
    return {
      valido: false,
      bloqueado: true,
      mensagem: 'Defina a pasta de origem para iniciar a triagem.',
      cor: 'var(--gs-warn)',
      bg: 'rgba(232, 104, 40, 0.1)',
      borda: 'rgba(232, 104, 40, 0.3)',
    };
  }

  if (!d) {
    return {
      valido: false,
      bloqueado: true,
      mensagem: 'Defina uma pasta de destino separada para as fotos organizadas.',
      cor: 'var(--gs-warn)',
      bg: 'rgba(232, 104, 40, 0.1)',
      borda: 'rgba(232, 104, 40, 0.3)',
    };
  }

  if (o === d) {
    return {
      valido: false,
      bloqueado: true,
      mensagem: 'RECUSADO: Origem e destino sao a mesma pasta. Escolha uma pasta de destino diferente para evitar sobreposicao e mistura de arquivos.',
      cor: 'var(--gs-error)',
      bg: 'rgba(248, 113, 113, 0.12)',
      borda: 'rgba(248, 113, 113, 0.4)',
    };
  }

  if (d.startsWith(o + '/')) {
    return {
      valido: false,
      bloqueado: true,
      mensagem: 'RECUSADO: A pasta de destino esta dentro da pasta de origem. Escolha pastas independentes para evitar ciclo de varredura.',
      cor: 'var(--gs-error)',
      bg: 'rgba(248, 113, 113, 0.12)',
      borda: 'rgba(248, 113, 113, 0.4)',
    };
  }

  if (o.startsWith(d + '/')) {
    return {
      valido: false,
      bloqueado: true,
      mensagem: 'RECUSADO: A pasta de origem esta dentro da pasta de destino. Escolha pastas independentes.',
      cor: 'var(--gs-error)',
      bg: 'rgba(248, 113, 113, 0.12)',
      borda: 'rgba(248, 113, 113, 0.4)',
    };
  }

  return {
    valido: true,
    bloqueado: false,
    mensagem: 'Pastas validas e independentes. Pronto para iniciar.',
    cor: 'var(--gs-ok)',
    bg: 'rgba(240, 240, 240, 0.1)',
    borda: 'rgba(240, 240, 240, 0.3)',
  };
}

function atualizarPainelValidacao() {
  const o = setupEls.inputOrigem ? setupEls.inputOrigem.value.trim() : '';
  const d = setupEls.inputDestino ? setupEls.inputDestino.value.trim() : '';

  if (setupEls.origemPreview) {
    setupEls.origemPreview.textContent = o || '(Nenhuma pasta selecionada)';
    setupEls.origemPreview.style.color = o ? 'var(--eden-bone)' : 'var(--gs-warn)';
  }

  if (setupEls.destinoPreview) {
    setupEls.destinoPreview.textContent = d || '(Nenhuma pasta selecionada)';
    setupEls.destinoPreview.style.color = d ? 'var(--eden-bone)' : 'var(--gs-warn)';
  }

  const checagem = verificarCaminhos(o, d);
  if (setupEls.statusAlerta) {
    setupEls.statusAlerta.textContent = checagem.mensagem;
    setupEls.statusAlerta.style.color = checagem.cor;
    setupEls.statusAlerta.style.background = checagem.bg;
    setupEls.statusAlerta.style.borderColor = checagem.borda;
  }
}

function renderSugestoes(paths) {
  if (!setupEls.sugestoesBox || !setupEls.sugestoesLista) return;
  if (!paths || paths.length === 0) {
    setupEls.sugestoesBox.style.display = 'none';
    return;
  }

  setupEls.sugestoesLista.innerHTML = '';
  paths.forEach((caminho) => {
    const item = document.createElement('div');
    item.className = 'sugestao-item-linha';
    item.style.display = 'flex';
    item.style.alignItems = 'center';
    item.style.justifyContent = 'space-between';
    item.style.gap = '8px';
    item.style.padding = '8px 10px';
    item.style.background = 'rgba(255, 255, 255, 0.02)';
    item.style.borderRadius = '6px';
    item.style.border = '1px solid var(--eden-border)';

    const pathSpan = document.createElement('span');
    pathSpan.style.fontFamily = 'var(--font-mono)';
    pathSpan.style.fontSize = '11px';
    pathSpan.style.color = 'var(--eden-bone-dim)';
    pathSpan.style.wordBreak = 'break-all';
    pathSpan.textContent = caminho;

    const btnsDiv = document.createElement('div');
    btnsDiv.style.display = 'flex';
    btnsDiv.style.gap = '6px';
    btnsDiv.style.flexShrink = '0';

    const btnOrigem = document.createElement('button');
    btnOrigem.className = 'btn-browse';
    btnOrigem.style.padding = '4px 8px';
    btnOrigem.style.fontSize = '10px';
    btnOrigem.textContent = 'Usar Origem';
    btnOrigem.addEventListener('click', () => {
      setupEls.inputOrigem.value = caminho;
      state.origem = caminho;
      atualizarPainelValidacao();
      showToast(`Origem definida: ${caminho}`);
    });

    const btnDestino = document.createElement('button');
    btnDestino.className = 'btn-browse';
    btnDestino.style.padding = '4px 8px';
    btnDestino.style.fontSize = '10px';
    btnDestino.textContent = 'Usar Destino';
    btnDestino.addEventListener('click', () => {
      setupEls.inputDestino.value = caminho;
      state.destino = caminho;
      atualizarPainelValidacao();
      showToast(`Destino definido: ${caminho}`);
    });

    btnsDiv.appendChild(btnOrigem);
    btnsDiv.appendChild(btnDestino);
    item.appendChild(pathSpan);
    item.appendChild(btnsDiv);
    setupEls.sugestoesLista.appendChild(item);
  });

  setupEls.sugestoesBox.style.display = 'block';
}

// Inicializacao: nunca pre-preenche caminhos reais ativos automaticamente
(async () => {
  diagLog('INFO', 'Inicializando GS Photo Triage...', {
    platform: navigator.platform,
    userAgent: navigator.userAgent,
    isTauri,
  });

  setupEls.inputOrigem.value = '';
  setupEls.inputDestino.value = '';
  state.origem = '';
  state.destino = '';
  atualizarPainelValidacao();

  if (isTauri) {
    try {
      const paths = await safeInvoke('get_default_paths');
      if (Array.isArray(paths) && paths.length > 0) {
        renderSugestoes(paths);
        diagLog('INFO', `Sugestoes carregadas: ${paths.length} caminho(s)`);
      }
    } catch (e) {
      diagLog('WARN', 'Nao foi possivel obter sugestoes de caminho:', e);
    }
  } else {
    const isMobile = /Android|iPhone|iPad/i.test(navigator.userAgent);
    const sugestoesWeb = isMobile
      ? ['/storage/emulated/0/Download', '/storage/emulated/0/Pictures/Organizadas', '/storage/emulated/0/DCIM']
      : ['/home/joey/Pictures/Teste_Fototriagem/Google Photos', '/home/joey/Pictures/Teste_Fototriagem/Destino_Organizado'];
    renderSugestoes(sugestoesWeb);
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
function switchView(viewName, pushHistory = true) {
  if (pushHistory && state.view !== viewName) {
    history.pushState({ view: viewName }, '');
  }
  state.view = viewName;
  Object.keys(views).forEach((k) => {
    views[k].classList.toggle('active', k === viewName);
  });

  if (viewName === 'months') {
    renderMonthsList();
  } else if (viewName === 'triage') {
    renderTriageView();
  } else if (viewName === 'explorer') {
    const defaultRoot = /Android|iPhone|iPad/i.test(navigator.userAgent) ? '/storage/emulated/0' : '/home/joey/Pictures';
    loadExplorer(state.explorerPath || state.origem || defaultRoot);
  } else if (viewName === 'setup') {
    atualizarPainelValidacao();
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
        atualizarPainelValidacao();
      }
    } catch {
      const p = prompt('Digite o caminho da pasta de fotos:', setupEls.inputOrigem.value);
      if (p) {
        setupEls.inputOrigem.value = p;
        state.origem = p;
        atualizarPainelValidacao();
      }
    }
  } else {
    const p = prompt('Digite o caminho da pasta de fotos:', '/home/joey/Pictures/Teste_Fototriagem/Google Photos');
    if (p) {
      setupEls.inputOrigem.value = p;
      state.origem = p;
      atualizarPainelValidacao();
    }
  }
});

setupEls.btnBrowseDestino.addEventListener('click', async () => {
  if (isTauri) {
    try {
      const selected = await openDialog({ directory: true, multiple: false, title: 'Selecione a Pasta de Destino' });
      if (selected) {
        setupEls.inputDestino.value = selected;
        state.destino = selected;
        atualizarPainelValidacao();
      }
    } catch {
      const p = prompt('Digite o caminho da pasta de destino:', setupEls.inputDestino.value);
      if (p) {
        setupEls.inputDestino.value = p;
        state.destino = p;
        atualizarPainelValidacao();
      }
    }
  } else {
    const p = prompt('Digite o caminho da pasta de destino:', '/home/joey/Pictures/Teste_Fototriagem/Destino_Organizado');
    if (p) {
      setupEls.inputDestino.value = p;
      state.destino = p;
      atualizarPainelValidacao();
    }
  }
});

setupEls.inputOrigem.addEventListener('input', () => {
  state.origem = setupEls.inputOrigem.value.trim();
  atualizarPainelValidacao();
});

setupEls.inputDestino.addEventListener('input', () => {
  state.destino = setupEls.inputDestino.value.trim();
  atualizarPainelValidacao();
});

setupEls.btnAbrirExplorador.addEventListener('click', () => {
  switchView('explorer');
});

setupEls.btnIniciar.addEventListener('click', pedirConfirmacaoScan);

if (setupEls.btnConfirmScanExecutar) {
  setupEls.btnConfirmScanExecutar.addEventListener('click', () => {
    fecharConfirmacaoScan(true);
    executarScan();
  });
}

if (setupEls.btnConfirmScanCancelar) {
  setupEls.btnConfirmScanCancelar.addEventListener('click', () => {
    fecharConfirmacaoScan(true);
  });
}

if (setupEls.modalConfirmScan) {
  setupEls.modalConfirmScan.addEventListener('click', (e) => {
    if (e.target === setupEls.modalConfirmScan) {
      fecharConfirmacaoScan(true);
    }
  });
}

// ==========================================================================
// IMMICH INTEGRATION LOGIC
// ==========================================================================
if (setupEls.tabLocal && setupEls.tabImmich) {
  setupEls.tabLocal.addEventListener('click', () => {
    state.mode = 'local';
    setupEls.tabLocal.style.background = 'var(--eden-gold)';
    setupEls.tabLocal.style.color = '#000';
    setupEls.tabLocal.style.borderColor = 'var(--eden-gold)';
    setupEls.tabImmich.style.background = 'transparent';
    setupEls.tabImmich.style.color = 'var(--eden-stone)';
    setupEls.tabImmich.style.borderColor = 'var(--eden-border)';
    setupEls.modeLocal.style.display = 'block';
    setupEls.modeImmich.style.display = 'none';
  });

  setupEls.tabImmich.addEventListener('click', () => {
    state.mode = 'immich';
    setupEls.tabImmich.style.background = 'var(--eden-gold)';
    setupEls.tabImmich.style.color = '#000';
    setupEls.tabImmich.style.borderColor = 'var(--eden-gold)';
    setupEls.tabLocal.style.background = 'transparent';
    setupEls.tabLocal.style.color = 'var(--eden-stone)';
    setupEls.tabLocal.style.borderColor = 'var(--eden-border)';
    setupEls.modeImmich.style.display = 'block';
    setupEls.modeLocal.style.display = 'none';
  });
}

if (setupEls.inputImmichUrl && state.immichUrl) {
  setupEls.inputImmichUrl.value = state.immichUrl;
}
if (setupEls.inputImmichKey && state.immichApiKey) {
  setupEls.inputImmichKey.value = state.immichApiKey;
}
if (state.immichApiKey && setupEls.tabImmich) {
  setupEls.tabImmich.click();
}

if (setupEls.btnImmichTest) {
  setupEls.btnImmichTest.addEventListener('click', async () => {
    const url = (setupEls.inputImmichUrl.value || '').trim();
    if (!url) {
      showToast('Informe a URL do Immich.');
      return;
    }
    setupEls.immichStatusText.textContent = 'Testando conexão com ' + url + '...';
    try {
      const ok = await safeInvoke('immich_test_connection', { serverUrl: url });
      if (ok) {
        setupEls.immichStatusText.innerHTML = '<span style="color:var(--gs-ok);">✓ Servidor Immich respondendo com sucesso!</span>';
        showToast('Conectado ao servidor Immich com sucesso!');
      } else {
        setupEls.immichStatusText.innerHTML = '<span style="color:var(--gs-error);">✗ Servidor respondeu com status inesperado.</span>';
      }
    } catch (e) {
      setupEls.immichStatusText.innerHTML = `<span style="color:var(--gs-error);">✗ Falha de conexão: ${e}</span>`;
      showToast(`Erro ao testar conexão: ${e}`);
    }
  });
}

if (setupEls.btnImmichConectar) {
  setupEls.btnImmichConectar.addEventListener('click', async () => {
    const url = (setupEls.inputImmichUrl.value || '').trim();
    const key = (setupEls.inputImmichKey.value || '').trim();
    if (!url) {
      showToast('Informe a URL do Immich.');
      return;
    }
    setupEls.btnImmichConectar.disabled = true;
    setupEls.btnImmichConectar.innerHTML = '<span>Carregando biblioteca...</span>';
    try {
      localStorage.setItem('immich_url', url);
      localStorage.setItem('immich_key', key);
      state.immichUrl = url;
      state.immichApiKey = key;
      const res = await safeInvoke('immich_connect', { serverUrl: url, apiKey: key });
      state.scanData = res;
      state.meses = res.meses || [];
      state.selectedMonthIndex = 0;
      showToast(`Carregados ${res.total_arquivos} arquivos do Immich!`);
      switchView('months');
    } catch (e) {
      showToast(`Erro ao conectar ao Immich: ${e}`);
      setupEls.immichStatusText.innerHTML = `<span style="color:var(--gs-error);">✗ ${e}</span>`;
    } finally {
      setupEls.btnImmichConectar.disabled = false;
      setupEls.btnImmichConectar.innerHTML = '<span>Conectar e Triar Immich</span><span style="font-size: 11px; opacity: 0.8;">[ ☁️ NUVEM ]</span>';
    }
  });
}

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
      showToast('Navegação no sistema de arquivos disponível apenas no app.');
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
        <div class="month-name" style="font-size: 15px;">📁 ${escapeHtml(sub.nome)}</div>
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
      atualizarPainelValidacao();
      showToast(`Origem definida: ${sub.nome}`);
      switchView('setup');
    });

    card.querySelector('.btn-set-destino-sub').addEventListener('click', (e) => {
      e.stopPropagation();
      setupEls.inputDestino.value = sub.caminho;
      state.destino = sub.caminho;
      atualizarPainelValidacao();
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
    atualizarPainelValidacao();
    showToast(`Origem definida: ${state.explorerPath}`);
    switchView('setup');
  }
});

explorerEls.btnSetDestino.addEventListener('click', () => {
  if (state.explorerPath) {
    setupEls.inputDestino.value = state.explorerPath;
    state.destino = state.explorerPath;
    atualizarPainelValidacao();
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

function pedirConfirmacaoScan() {
  const origem = setupEls.inputOrigem ? setupEls.inputOrigem.value.trim() : '';
  const destino = setupEls.inputDestino ? setupEls.inputDestino.value.trim() : '';
  const checagem = verificarCaminhos(origem, destino);

  if (!checagem.valido) {
    showToast(checagem.mensagem);
    atualizarPainelValidacao();
    return;
  }

  if (setupEls.modalConfirmOrigemText) {
    setupEls.modalConfirmOrigemText.textContent = origem;
  }
  if (setupEls.modalConfirmDestinoText) {
    setupEls.modalConfirmDestinoText.textContent = destino;
  }
  if (setupEls.modalConfirmScan) {
    setupEls.modalConfirmScan.classList.add('active');
    history.pushState({ modal: 'confirm_scan' }, '');
  }
}

function fecharConfirmacaoScan(triggerBack = false) {
  if (setupEls.modalConfirmScan) {
    setupEls.modalConfirmScan.classList.remove('active');
  }
  if (triggerBack && history.state && history.state.modal === 'confirm_scan') {
    history.back();
  }
}

async function executarScan() {
  const origem = setupEls.inputOrigem ? setupEls.inputOrigem.value.trim() : '';
  const destino = setupEls.inputDestino ? setupEls.inputDestino.value.trim() : '';
  const especiais = setupEls.chkEspeciais ? setupEls.chkEspeciais.checked : false;

  const checagem = verificarCaminhos(origem, destino);
  if (!checagem.valido) {
    showToast(checagem.mensagem);
    atualizarPainelValidacao();
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

// Sondagem leve de dimensoes sem decodificacao total (evita img.decode() que estoura RAM em 48MP)
function obterDimensoesImagemSegura(caminho) {
  return new Promise((resolve) => {
    const url = getMediaUrl(caminho);
    const probe = new Image();
    probe.decoding = 'async';
    let finalizado = false;

    const concluir = (largura, altura) => {
      if (finalizado) return;
      finalizado = true;
      probe.onload = null;
      probe.onerror = null;
      probe.removeAttribute('src');
      probe.src = '';
      resolve({ largura, altura });
    };

    probe.onload = () => {
      concluir(probe.naturalWidth || 0, probe.naturalHeight || 0);
    };
    probe.onerror = () => {
      concluir(0, 0);
    };
    probe.src = url;
  });
}

function limparImagemAtiva() {
  if (triageEls.activeImg) {
    triageEls.activeImg.onload = null;
    triageEls.activeImg.onerror = null;
    triageEls.activeImg.removeAttribute('src');
    triageEls.activeImg.src = '';
    triageEls.activeImg.style.display = 'none';
  }
}

function limparCardsLaterais() {
  if (triageEls.cardPrev) triageEls.cardPrev.innerHTML = '<div class="card-flanking-empty">◄ INICIO</div>';
  if (triageEls.cardNext) triageEls.cardNext.innerHTML = '<div class="card-flanking-empty">FIM ►</div>';
}

function limparTodasMidias() {
  limparImagemAtiva();
  stopActiveVideo();
  limparCardsLaterais();
}

// ==========================================================================
// MONTHS VIEW LOGIC
// ==========================================================================
monthsEls.btnBack.addEventListener('click', () => switchView('setup'));

// ==========================================================================
// PASTAS DE DESCARTE
// `_lixo` e `_copias` sao as unicas pastas que o app enche e nunca esvazia
// sozinho. No desktop isso e irrelevante; num celular e o assunto, porque
// liberar espaco e o motivo de existir do fluxo. Aqui elas viram numero, e o
// esvaziamento e uma coluna de opcoes com confirmacao, como todo o resto.
// ==========================================================================

const descarteEls = {
  painel: document.getElementById('descarte-painel'),
};

const DESCARTE_ROTULO = {
  lixo: '_lixo',
  copias: '_copias',
};

async function atualizarDescarte() {
  if (!descarteEls.painel) return;
  if (!isTauri) {
    descarteEls.painel.hidden = true;
    return;
  }

  let info;
  try {
    info = await safeInvoke('descarte_stats');
  } catch (err) {
    diagLog('WARN', `descarte_stats falhou: ${err}`);
    descarteEls.painel.hidden = true;
    return;
  }

  const comConteudo = (info || []).filter((d) => d.arquivos > 0);
  if (comConteudo.length === 0) {
    descarteEls.painel.hidden = true;
    descarteEls.painel.innerHTML = '';
    return;
  }

  descarteEls.painel.hidden = false;
  descarteEls.painel.innerHTML = comConteudo
    .map(
      (d) => `
      <div class="descarte-linha" data-qual="${d.qual}">
        <span class="descarte-nome">${DESCARTE_ROTULO[d.qual] || d.qual}</span>
        <span class="descarte-medida">${d.arquivos} arquivo${d.arquivos > 1 ? 's' : ''} · ${d.formatado}</span>
        <div class="descarte-acoes">
          <button type="button" class="descarte-opcao" data-acao="pedir" data-qual="${d.qual}">
            Esvaziar ${DESCARTE_ROTULO[d.qual] || d.qual} e liberar ${d.formatado}
          </button>
        </div>
      </div>`
    )
    .join('');
}

/// Troca o botao pela confirmacao, que e uma coluna de duas opcoes. Apagar e
/// a unica coisa irreversivel do app, entao ela nunca fica a um toque so.
function pedirConfirmacaoDescarte(qual) {
  const linha = descarteEls.painel.querySelector(`.descarte-linha[data-qual="${qual}"]`);
  if (!linha) return;
  const medida = linha.querySelector('.descarte-medida')?.textContent || '';
  const acoes = linha.querySelector('.descarte-acoes');
  acoes.innerHTML = `
    <button type="button" class="descarte-opcao perigo" data-acao="confirmar" data-qual="${qual}">
      Apagar de vez ${medida}. Isto nao tem desfazer.
    </button>
    <button type="button" class="descarte-opcao" data-acao="cancelar" data-qual="${qual}">
      Cancelar [ESC]
    </button>`;
  acoes.querySelector('[data-acao="cancelar"]').focus();
}

async function esvaziarDescarte(qual) {
  try {
    const r = await safeInvoke('esvaziar_descarte', { qual });
    let msg = `${DESCARTE_ROTULO[qual] || qual} esvaziado: ${r.arquivos_removidos} arquivo(s), ${r.formatado} liberado(s)`;
    if (r.desfazeres_perdidos > 0) {
      msg += `. ${r.desfazeres_perdidos} desfazer(es) deixaram de existir junto`;
    }
    showToast(msg);
    // As decisoes que sumiram junto com os arquivos saem tambem da conta que
    // a tela mostra, senao ela promete um desfazer que nao existe mais.
    state.undoCount = Math.max(0, state.undoCount - (r.desfazeres_perdidos || 0));
    if (triageEls.undoCount) {
      triageEls.undoCount.textContent = `(${state.undoCount})`;
    }
  } catch (err) {
    showToast(`Erro ao esvaziar: ${err}`);
  }
  await atualizarDescarte();
}

if (descarteEls.painel) {
  descarteEls.painel.addEventListener('click', (e) => {
    const btn = e.target.closest('button[data-acao]');
    if (!btn) return;
    const { acao, qual } = btn.dataset;
    if (acao === 'pedir') pedirConfirmacaoDescarte(qual);
    else if (acao === 'cancelar') atualizarDescarte();
    else if (acao === 'confirmar') esvaziarDescarte(qual);
  });

  // ESC desiste da confirmacao antes de sair da tela, como em todo o resto.
  descarteEls.painel.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && descarteEls.painel.querySelector('[data-acao="confirmar"]')) {
      e.preventDefault();
      e.stopPropagation();
      atualizarDescarte();
    }
  });
}

function renderMonthsList() {
  atualizarDescarte();
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
  limparTodasMidias();
  if (isTauri) {
    try {
      const items = await safeInvoke('get_month_items', {
        ano: monthData.ano,
        mes: monthData.mes,
      });
      state.fila = items.filter((i) => !i.triada);
      diagLog('INFO', `Mês aberto: ${monthData.nome_mes} ${monthData.ano} (${state.fila.length} itens a triar)`);
    } catch (err) {
      showToast(`Erro ao abrir mês: ${err}`);
      diagLog('ERROR', `get_month_items falhou: ${err}`);
      return;
    }
  } else {
    showToast('Abertura de fotos disponível apenas no app.');
    return;
  }

  state.pos = 0;
  switchView('triage');
}

// ==========================================================================
// TRIAGE VIEW LOGIC (CAROUSEL & MECHANICAL CONTROLLER)
// ==========================================================================

function renderTriageView() {
  if (!state.currentMonth) return;

  triageEls.monthTitle.textContent = `${state.currentMonth.nome_mes} ${state.currentMonth.ano}`;
  triageEls.undoCount.textContent = `(${state.undoCount})`;

  const total = state.fila.length;
  if (total === 0) {
    // Month is finished
    limparTodasMidias();
    triageEls.pos.textContent = '0 / 0';
    triageEls.date.textContent = 'Concluído';
    triageEls.tag.style.display = 'none';
    triageEls.copies.style.display = 'none';
    triageEls.activeEmpty.style.display = 'flex';
    return;
  }

  state.pos = Math.max(0, Math.min(state.pos, total - 1));
  triageEls.activeEmpty.style.display = 'none';
  const atual = state.fila[state.pos];
  if (!atual) return;

  // Header & Meta
  triageEls.pos.textContent = `${state.pos + 1} / ${total}`;
  triageEls.date.textContent = atual.data_formatada || 'Sem data';

  // Tag de origem da data
  const orig = atual.origem_data || 'desconhecida';
  triageEls.tag.textContent = orig.toUpperCase();
  triageEls.tag.className = `tag-origem tag-${orig}`;
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
  const activeUrl = getMediaUrl(atual.caminho);
  if (atual.eh_video) {
    stopActiveVideo();
    limparImagemAtiva();
    triageEls.activeVideo.style.display = 'block';
    triageEls.activeVideo.src = activeUrl;
    triageEls.activeVideo.play().catch(() => {});
  } else if (atual.caminho && atual.caminho.startsWith('immich://')) {
    stopActiveVideo();
    limparImagemAtiva();
    triageEls.activeImg.style.display = 'block';
    const thisLoadToken = ++state.imageLoadSeq;
    safeInvoke('load_image_data', { path: atual.caminho }).then((b64) => {
      if (state.imageLoadSeq === thisLoadToken) {
        triageEls.activeImg.src = b64;
      }
    }).catch((err) => {
      diagLog('ERROR', `Falha ao carregar imagem do Immich: ${err}`);
    });
  } else {
    stopActiveVideo();
    limparImagemAtiva();
    triageEls.activeImg.decoding = 'async';
    triageEls.activeImg.loading = 'eager';
    triageEls.activeImg.style.display = 'block';
    const thisLoadToken = ++state.imageLoadSeq;
    triageEls.activeImg.onerror = async () => {
      triageEls.activeImg.onerror = null;
      if (state.imageLoadSeq !== thisLoadToken) return;
      try {
        diagLog('WARN', `Asset streaming falhou para ${atual.caminho}, tentando load_image_data...`);
        const b64 = await safeInvoke('load_image_data', { path: atual.caminho });
        if (state.imageLoadSeq === thisLoadToken) {
          triageEls.activeImg.src = b64;
        }
      } catch (err) {
        diagLog('ERROR', `Falha ao carregar imagem ${atual.caminho}: ${err}`);
      }
    };
    triageEls.activeImg.src = activeUrl;
  }

  // Previews laterais: em telas mobile / estreitas, nao carrega imagens ocultas em memoria
  const isNarrow = window.innerWidth <= 768;
  if (isNarrow) {
    triageEls.cardPrev.innerHTML = '<div class="card-flanking-empty">◄ ANTERIOR</div>';
    triageEls.cardNext.innerHTML = '<div class="card-flanking-empty">PRÓXIMO ►</div>';
  } else {
    // Left Flanking Preview
    if (state.pos > 0) {
      const prev = state.fila[state.pos - 1];
      triageEls.cardPrev.innerHTML = `<img src="${getMediaUrl(prev.caminho)}" alt="Anterior" loading="lazy" decoding="async" />`;
    } else {
      triageEls.cardPrev.innerHTML = '<div class="card-flanking-empty">◄ INÍCIO</div>';
    }

    // Right Flanking Preview
    if (state.pos < total - 1) {
      const next = state.fila[state.pos + 1];
      triageEls.cardNext.innerHTML = `<img src="${getMediaUrl(next.caminho)}" alt="Próximo" loading="lazy" decoding="async" />`;
    } else {
      triageEls.cardNext.innerHTML = '<div class="card-flanking-empty">FIM ►</div>';
    }
  }
}

// Navigation & Actions
function navigateLeft() {
  if (state.isActionBusy || state.pos <= 0) return;
  animateKey(triageEls.keyLeft);
  triageEls.cardCenter.classList.add('animate-right');
  limparImagemAtiva();
  stopActiveVideo();
  setTimeout(() => {
    state.pos = Math.max(0, state.pos - 1);
    renderTriageView();
    triageEls.cardCenter.classList.remove('animate-right');
  }, 120);
}

function navigateRight() {
  if (state.isActionBusy || state.pos >= state.fila.length - 1) return;
  animateKey(triageEls.keyRight);
  triageEls.cardCenter.classList.add('animate-left');
  limparImagemAtiva();
  stopActiveVideo();
  setTimeout(() => {
    state.pos = Math.min(state.fila.length - 1, state.pos + 1);
    renderTriageView();
    triageEls.cardCenter.classList.remove('animate-left');
  }, 120);
}

async function actionTrash() {
  if (state.isActionBusy || state.fila.length === 0) return;
  state.isActionBusy = true;
  const midia = state.fila[state.pos];
  animateKey(triageEls.keyUp);
  triageEls.cardCenter.classList.add('animate-trash');
  limparImagemAtiva();
  stopActiveVideo();

  setTimeout(async () => {
    try {
      if (isTauri) {
        await safeInvoke('trash_item', { itemPath: midia.caminho });
      }
      state.undoCount++;
      const idx = state.fila.findIndex((i) => i.caminho === midia.caminho);
      if (idx !== -1) {
        state.fila.splice(idx, 1);
        state.pos = Math.max(0, Math.min(state.pos, state.fila.length - 1));
      }
      showToast(`🗑️ ${midia.nome} movido para _lixo`);
      updateActionBanner(`${midia.nome} → _lixo`);
      updateMonthStats();
      renderTriageView();
    } catch (err) {
      showToast(`Erro ao mover para o lixo: ${err}`);
    } finally {
      triageEls.cardCenter.classList.remove('animate-trash');
      state.isActionBusy = false;
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

function stopActiveVideo() {
  if (triageEls.activeVideo) {
    triageEls.activeVideo.pause();
    triageEls.activeVideo.removeAttribute('src');
    try { triageEls.activeVideo.load(); } catch (_) {}
    triageEls.activeVideo.style.display = 'none';
  }
}

function toggleVideoPlayback() {
  if (triageEls.activeVideo && triageEls.activeVideo.style.display !== 'none') {
    if (triageEls.activeVideo.paused) {
      triageEls.activeVideo.play().catch(() => {});
    } else {
      triageEls.activeVideo.pause();
    }
  }
}

function returnToMonths(pushHist = true) {
  limparTodasMidias();
  updateMonthStats();
  switchView('months', pushHist);
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
let activePointerId = null;
let touchStartX = 0;
let touchStartY = 0;
let touchStartTime = 0;

function resetPointerTracking() {
  activePointerId = null;
  touchStartX = 0;
  touchStartY = 0;
  touchStartTime = 0;
}

if (triageEls.cardCenter) {
  triageEls.cardCenter.addEventListener('pointerdown', (e) => {
    // Only track primary pointer to prevent multi-finger pinch collisions
    if (activePointerId !== null) return;
    activePointerId = e.pointerId;
    try {
      triageEls.cardCenter.setPointerCapture(e.pointerId);
    } catch (_) {}
    touchStartX = e.clientX;
    touchStartY = e.clientY;
    touchStartTime = Date.now();
  });

  triageEls.cardCenter.addEventListener('pointercancel', (e) => {
    if (e.pointerId === activePointerId) {
      try {
        triageEls.cardCenter.releasePointerCapture(e.pointerId);
      } catch (_) {}
      resetPointerTracking();
    }
  });

  triageEls.cardCenter.addEventListener('pointerup', (e) => {
    if (e.pointerId !== activePointerId) return;
    try {
      triageEls.cardCenter.releasePointerCapture(e.pointerId);
    } catch (_) {}

    const dt = Date.now() - touchStartTime;
    const startX = touchStartX;
    const startY = touchStartY;
    resetPointerTracking();

    if (dt > 1200) return;

    const diffX = e.clientX - startX;
    const diffY = e.clientY - startY;
    const absX = Math.abs(diffX);
    const absY = Math.abs(diffY);

    // Tap on video: toggle play/pause
    if (absX < 15 && absY < 15) {
      if (triageEls.activeVideo && triageEls.activeVideo.style.display !== 'none') {
        toggleVideoPlayback();
      }
      return;
    }

    // Ignore touches starting at top/bottom edges (Android notification bar / system gestures)
    const isTopBottomEdge = startY < 36 || startY > window.innerHeight - 44;
    if (isTopBottomEdge) return;

    // Swipe Up -> Trash (> 70px vertical)
    if (diffY < -70 && absY > absX * 1.2) {
      actionTrash();
      return;
    }

    // Swipe Down -> Folder Modal (> 70px vertical)
    if (diffY > 70 && absY > absX * 1.2) {
      openFolderModal();
      return;
    }

    // Swipe Horizontal (> 60px horizontal, ignoring side edges that conflict with Android system back gesture)
    const isSideEdge = startX < 24 || startX > window.innerWidth - 24;
    if (!isSideEdge && absX > 60 && absX > absY * 1.2) {
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
    try {
      state.folders = await safeInvoke('get_target_folders');
    } catch (err) {
      diagLog('WARN', `Erro ao buscar pastas de destino: ${err}`);
      state.folders = [];
    }
  } else {
    state.folders = [];
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
  history.pushState({ modal: 'folder' }, '');
}

function closeFolderModal(triggerBack = false) {
  modalEls.overlay.classList.remove('active');
  modalEls.inputName.value = '';
  if (triggerBack && history.state && history.state.modal === 'folder') {
    history.back();
  }
}

if (modalEls.overlay) {
  modalEls.overlay.addEventListener('click', (e) => {
    if (e.target === modalEls.overlay) {
      closeFolderModal(true);
    }
  });
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
    item.innerHTML = `<span>📁  ${escapeHtml(f.name)}</span><span style="font-size: 11px; opacity: 0.6;">[⏎ Mover]</span>`;
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
    item.innerHTML = `<span>📁  ${escapeHtml(f.name)}</span><span style="font-size: 11px; opacity: 0.6;">[⏎ Renomear]</span>`;
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
  if (state.isActionBusy || state.fila.length === 0) return;
  state.isActionBusy = true;
  const midia = state.fila[state.pos];
  closeFolderModal();

  triageEls.cardCenter.classList.add('animate-move');
  limparImagemAtiva();
  stopActiveVideo();
  setTimeout(async () => {
    try {
      if (isTauri) {
        await safeInvoke('move_item', {
          itemPath: midia.caminho,
          targetFolderName: folderName,
        });
      }
      state.undoCount++;
      const idx = state.fila.findIndex((i) => i.caminho === midia.caminho);
      if (idx !== -1) {
        state.fila.splice(idx, 1);
        state.pos = Math.max(0, Math.min(state.pos, state.fila.length - 1));
      }
      showToast(`📁 ${midia.nome} ➔ ${folderName}`);
      updateActionBanner(`${midia.nome} → ${folderName}`);
      updateMonthStats();
      renderTriageView();
    } catch (err) {
      showToast(`Erro ao mover: ${err}`);
    } finally {
      triageEls.cardCenter.classList.remove('animate-move');
      state.isActionBusy = false;
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
    if (setupEls.modalConfirmScan && setupEls.modalConfirmScan.classList.contains('active')) {
      if (e.key === 'Escape') {
        e.preventDefault();
        fecharConfirmacaoScan(true);
      } else if (e.key === 'Enter') {
        e.preventDefault();
        fecharConfirmacaoScan(true);
        executarScan();
      }
      return;
    }
    if (e.key === 'Enter' && !setupEls.btnIniciar.disabled && !isInputFocused) {
      e.preventDefault();
      pedirConfirmacaoScan();
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

// ==========================================================================
// SYSTEM BACK NAVIGATION (Android Back Button / Back Gesture Support)
// ==========================================================================
window.addEventListener('popstate', () => {
  // 1. Se modal de confirmacao de scan estiver aberto, fecha
  if (setupEls.modalConfirmScan && setupEls.modalConfirmScan.classList.contains('active')) {
    fecharConfirmacaoScan(false);
    return;
  }
  // 2. If folder modal is open, close it
  if (modalEls.overlay && modalEls.overlay.classList.contains('active')) {
    closeFolderModal(false);
    return;
  }
  // 2. If diagnostics drawer is open, close it
  if (diagEls.drawer && diagEls.drawer.style.display === 'flex') {
    diagEls.drawer.style.display = 'none';
    return;
  }
  // 3. If in triage view, return to months view
  if (state.view === 'triage') {
    returnToMonths(false);
    return;
  }
  // 4. If in explorer view, go to parent directory or return to setup
  if (state.view === 'explorer') {
    if (state.explorerData && state.explorerData.caminho_pai) {
      loadExplorer(state.explorerData.caminho_pai);
    } else {
      switchView('setup', false);
    }
    return;
  }
  // 5. If in months view, return to setup view
  if (state.view === 'months') {
    switchView('setup', false);
    return;
  }
});
