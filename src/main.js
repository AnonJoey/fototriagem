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
};

// Check if running inside Tauri
const isTauri = window.__TAURI_INTERNALS__ !== undefined;

// DOM Elements
const views = {
  setup: document.getElementById('view-setup'),
  months: document.getElementById('view-months'),
  triage: document.getElementById('view-triage'),
};

const setupEls = {
  inputOrigem: document.getElementById('input-origem'),
  btnBrowseOrigem: document.getElementById('btn-browse-origem'),
  inputDestino: document.getElementById('input-destino'),
  btnBrowseDestino: document.getElementById('btn-browse-destino'),
  chkEspeciais: document.getElementById('chk-especiais'),
  btnIniciar: document.getElementById('btn-iniciar-scan'),
};

const monthsEls = {
  container: document.getElementById('months-list-container'),
  summary: document.getElementById('months-summary-label'),
  btnBack: document.getElementById('btn-voltar-setup'),
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
  btnUndo: document.getElementById('btn-action-undo'),
  undoCount: document.getElementById('undo-count'),
  btnFolders: document.getElementById('btn-action-folders'),
  btnMenu: document.getElementById('btn-action-menu'),
  keyUp: document.getElementById('key-up'),
  keyDown: document.getElementById('key-down'),
  keyLeft: document.getElementById('key-left'),
  keyRight: document.getElementById('key-right'),
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

// Image Base64 Cache
const mediaCache = new Map();

async function getMediaDataUrl(path) {
  if (!path) return '';
  if (mediaCache.has(path)) {
    return mediaCache.get(path);
  }
  if (isTauri) {
    try {
      const dataUrl = await invoke('load_image_data', { path });
      mediaCache.set(path, dataUrl);
      return dataUrl;
    } catch {
      return convertFileSrc(path);
    }
  }
  return path;
}

// Pre-fill paths if available
(async () => {
  if (isTauri) {
    try {
      const paths = await invoke('get_default_paths');
      if (paths && paths.length >= 2) {
        setupEls.inputOrigem.value = paths[0];
        setupEls.inputDestino.value = paths[1];
        state.origem = paths[0];
        state.destino = paths[1];
      }
    } catch (e) {
      console.warn('Could not fetch default paths:', e);
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

setupEls.btnIniciar.addEventListener('click', startScan);

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
      const res = await invoke('scan_library', {
        origemPath: origem,
        destinoPath: destino ? destino : null,
        incluirEspeciais: especiais,
      });
      state.scanData = res;
      state.origem = origem;
      state.destino = destino;
      state.meses = res.meses;
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
    const items = await invoke('get_month_items', {
      ano: monthData.ano,
      mes: monthData.mes,
    });
    state.fila = items.filter((i) => !i.triada);
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

  if (atual.copias && atual.copias.length > 0) {
    triageEls.copies.textContent = `+${atual.copias.length} CÓPIA(S) ORIGEM`;
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
        await invoke('trash_item', { itemPath: midia.caminho });
      }
      state.undoCount++;
      state.fila.splice(state.pos, 1);
      showToast(`🗑️ ${midia.nome} movido para _lixo`);
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
  try {
    if (isTauri) {
      const restored = await invoke('undo_last_action');
      if (!restored) {
        showToast('Nada a desfazer.');
        return;
      }
      state.undoCount = Math.max(0, state.undoCount - 1);
      if (restored.ano === state.currentMonth.ano && restored.mes === state.currentMonth.mes) {
        state.fila.splice(state.pos, 0, restored);
        renderTriageView();
      }
      showToast(`Restaurado: ${restored.nome}`);
      updateMonthStats();
    } else {
      showToast('Desfazer simulado.');
    }
  } catch (err) {
    showToast(`Erro ao desfazer: ${err}`);
  }
}

async function updateMonthStats() {
  if (isTauri) {
    const meses = await invoke('get_months');
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
triageEls.btnFolders.addEventListener('click', openFolderModal);
triageEls.btnMenu.addEventListener('click', returnToMonths);
triageEls.btnEmptyReturn.addEventListener('click', returnToMonths);

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
    state.folders = await invoke('get_target_folders');
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
        await invoke('move_item', {
          itemPath: midia.caminho,
          targetFolderName: folderName,
        });
      }
      state.undoCount++;
      state.fila.splice(state.pos, 1);
      showToast(`📁 ${midia.nome} ➔ ${folderName}`);
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
        await invoke('create_target_folder', { name: text });
      }
      await executeMoveToFolder(text);
    } catch (err) {
      showToast(`Erro ao criar pasta: ${err}`);
    }
  } else if (state.modalMode === 'input_rename') {
    try {
      if (isTauri) {
        await invoke('rename_target_folder', {
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
      state.selectedFolderIndex = (state.selectedFolderIndex - 1 + max) % max;
      if (state.modalMode === 'choose_rename') renderRenameChoiceList();
      else renderFolderList();
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      const max = modalEls.list.children.length;
      state.selectedFolderIndex = (state.selectedFolderIndex + 1) % max;
      if (state.modalMode === 'choose_rename') renderRenameChoiceList();
      else renderFolderList();
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
    if (e.key === 'Enter' && !setupEls.btnIniciar.disabled) {
      e.preventDefault();
      startScan();
    }
    return;
  }

  // 3. Months View
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

  // 4. Triage View (Inspo Layout)
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
    } else if (e.key.toLowerCase() === 'z' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      actionUndo();
    }
  }
});
