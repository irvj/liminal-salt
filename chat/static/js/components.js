/**
 * Liminal Salt - Alpine.js Components
 * All Alpine components are registered here using Alpine.data().
 */

// =============================================================================
// Component Registration
// =============================================================================

document.addEventListener('alpine:init', () => {
    // Reusable Components
    Alpine.data('collapsibleSection', collapsibleSection);
    Alpine.data('selectDropdown', selectDropdown);
    Alpine.data('toastContainer', toastContainer);
    Alpine.data('confirmModal', confirmModal);

    // Modal Components
    Alpine.data('renameModal', renameModal);
    Alpine.data('scenarioModal', scenarioModal);
    Alpine.data('threadMemoryModal', threadMemoryModal);
    Alpine.data('editPersonaModal', editPersonaModal);
    Alpine.data('editPersonaModelModal', editPersonaModelModal);
    Alpine.data('contextFilesModal', contextFilesModal);

    // Page Components
    Alpine.data('sidebarState', sidebarState);
    Alpine.data('providerModelSettings', providerModelSettings);
    Alpine.data('homePersonaPicker', homePersonaPicker);
    Alpine.data('memoryPersonaPicker', memoryPersonaPicker);
    Alpine.data('personaSettingsPicker', personaSettingsPicker);
    Alpine.data('personaThreadDefaults', personaThreadDefaults);
    Alpine.data('providerPicker', providerPicker);
    Alpine.data('modelPicker', modelPicker);
    Alpine.data('themePicker', themePicker);
    Alpine.data('setupThemePicker', setupThemePicker);
    Alpine.data('themeModeToggle', themeModeToggle);
    Alpine.data('memorySettings', memorySettings);
    Alpine.data('contextHistoryLimit', contextHistoryLimit);
});

// =============================================================================
// Reusable: Collapsible Section
// =============================================================================

/**
 * Simple collapsible section toggle.
 * @param {boolean} initiallyOpen - Whether the section starts open
 */
function collapsibleSection(initiallyOpen = true) {
    return {
        open: initiallyOpen
    };
}

// =============================================================================
// Reusable: Toast Container
// =============================================================================

/**
 * Global toast renderer. One instance lives in base.html and listens for
 * the 'toast' window event dispatched by showToast() (utils.js). Toasts
 * auto-dismiss after `duration` ms, or stay until manually closed when
 * `duration` is 0.
 */
function toastContainer() {
    return {
        toasts: [],
        _nextId: 1,

        init() {
            window.addEventListener('toast', (e) => this.show(e.detail || {}));
        },

        show({ message, type = 'info', duration = 3000 }) {
            if (!message) return;
            const id = this._nextId++;
            this.toasts.push({ id, message, type });
            if (duration > 0) {
                setTimeout(() => this.dismiss(id), duration);
            }
        },

        dismiss(id) {
            this.toasts = this.toasts.filter(t => t.id !== id);
        }
    };
}

// =============================================================================
// Reusable: Confirm Dialog
// =============================================================================

/**
 * Global confirmation-dialog renderer. One instance lives in base.html and
 * listens for the 'confirm-dialog' window event dispatched by
 * confirmDialog() (utils.js). Resolves the caller's promise with true on
 * Confirm and false on Cancel / Esc / backdrop click.
 */
function confirmModal() {
    return {
        showModal: false,
        title: 'Confirm',
        message: '',
        confirmText: 'Confirm',
        cancelText: 'Cancel',
        confirmType: 'danger',
        _resolve: null,

        init() {
            window.addEventListener('confirm-dialog', (e) => this.open(e.detail || {}));
        },

        open({ title, message, confirmText, cancelText, confirmType, resolve }) {
            this.title = title || 'Confirm';
            this.message = message || 'Are you sure?';
            this.confirmText = confirmText || 'Confirm';
            this.cancelText = cancelText || 'Cancel';
            this.confirmType = confirmType || 'danger';
            this._resolve = resolve || null;
            this.showModal = true;
        },

        confirm() {
            const resolve = this._resolve;
            this._resolve = null;
            this.showModal = false;
            if (resolve) resolve(true);
        },

        cancel() {
            if (!this.showModal) return;
            const resolve = this._resolve;
            this._resolve = null;
            this.showModal = false;
            if (resolve) resolve(false);
        }
    };
}

// =============================================================================
// Sidebar State Component
// =============================================================================

function sidebarState() {
    return {
        collapsed: localStorage.getItem('sidebarCollapsed') === 'true',
        isMobile: window.innerWidth < 1024,
        isDark: localStorage.getItem('theme') !== 'light',

        async toggleTheme() {
            this.isDark = !this.isDark;
            const mode = this.isDark ? 'dark' : 'light';
            setTheme(mode);
            // Save preference to backend
            await saveThemePreference(getColorTheme(), mode);
        },

        init() {
            // Auto-collapse on smaller screens (< 1024px / lg breakpoint)
            if (this.isMobile) this.collapsed = true;

            // Listen for resize
            window.addEventListener('resize', () => {
                const wasMobile = this.isMobile;
                this.isMobile = window.innerWidth < 1024;
                // Auto-collapse when entering mobile
                if (this.isMobile && !wasMobile) this.collapsed = true;
                // Restore localStorage state when returning to desktop
                if (!this.isMobile && wasMobile) {
                    this.collapsed = localStorage.getItem('sidebarCollapsed') === 'true';
                }
            });

            // Listen for theme mode changes from other components
            window.addEventListener('theme-mode-changed', (e) => {
                this.isDark = e.detail.mode === 'dark';
            });

            // Persist collapsed state (desktop only)
            this.$watch('collapsed', val => {
                if (!this.isMobile) localStorage.setItem('sidebarCollapsed', val);
            });
        }
    };
}

// =============================================================================
// Rename Modal Component
// =============================================================================

function renameModal() {
    return {
        showModal: false,
        sessionId: '',
        newTitle: '',

        init() {
            window.addEventListener('open-rename-modal', (e) => {
                this.open(e.detail.id, e.detail.title);
            });
        },

        open(sessionId, currentTitle) {
            this.sessionId = sessionId;
            // Get current title from header (may have been updated dynamically)
            const headerTitle = document.getElementById('chat-title');
            this.newTitle = headerTitle ? headerTitle.textContent : currentTitle;
            this.showModal = true;
        }
    };
}

// =============================================================================
// Thread Memory Modal Component
// =============================================================================

function threadMemoryModal() {
    return {
        showModal: false,
        sessionId: '',
        content: '',
        updatedAt: '',
        running: false,
        statusMessage: '',
        statusType: '',
        // Settings state
        intervalMinutes: 0,
        messageFloor: 4,
        sizeLimit: 4000,
        hasOverride: false,
        settingsDirty: false,
        settingsSaving: false,
        _updateUrl: '',
        _statusUrl: '',
        _settingsSaveUrl: '',
        _settingsResetUrl: '',
        _pollHandle: null,
        _watchHandle: null,

        init() {
            this._updateUrl = this.$el.dataset.updateUrl;
            this._statusUrl = this.$el.dataset.statusUrl;
            this._settingsSaveUrl = this.$el.dataset.settingsSaveUrl;
            this._settingsResetUrl = this.$el.dataset.settingsResetUrl;
            window.addEventListener('open-thread-memory-modal', () => this.open());
            // Stop polling and background watcher when the modal closes by any path
            // (Close button, backdrop click, Esc, etc.).
            this.$watch('showModal', (open) => {
                if (!open) {
                    this._stopPolling();
                    this._stopWatch();
                }
            });
        },

        get formattedUpdatedAt() {
            if (!this.updatedAt) return '';
            try {
                const d = new Date(this.updatedAt);
                return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
                    + ' at ' + d.toLocaleTimeString('en-US', { hour: 'numeric', minute: '2-digit', hour12: true });
            } catch (e) {
                return this.updatedAt;
            }
        },

        _readFromDOM() {
            const source = document.getElementById('thread-memory-data');
            this.sessionId = source ? source.dataset.sessionId : '';
            this.content = source ? source.dataset.memory : '';
            this.updatedAt = source ? source.dataset.updatedAt : '';
            this.intervalMinutes = source ? parseInt(source.dataset.intervalMinutes) || 0 : 0;
            this.messageFloor = source ? parseInt(source.dataset.messageFloor) || 4 : 4;
            this.sizeLimit = source ? parseInt(source.dataset.sizeLimit) || 4000 : 4000;
            this.hasOverride = source ? source.dataset.hasOverride === 'true' : false;
            this.settingsDirty = false;
        },

        // Match server-side clamping (chat.views.chat.save_thread_memory_settings)
        // so the displayed value reflects what would be saved. Applied on @change
        // (blur) so the user sees the snap before they hit Save.
        clampInterval(v) {
            const n = Number.isFinite(v) ? v : 0;
            if (n <= 0) return 0;
            return Math.max(5, Math.min(1440, Math.round(n)));
        },
        clampMessageFloor(v) {
            const n = Number.isFinite(v) ? v : 4;
            return Math.max(1, Math.min(1000, Math.round(n)));
        },
        clampSizeLimit(v) {
            const n = Number.isFinite(v) ? v : 0;
            return Math.max(0, Math.min(100000, Math.round(n)));
        },

        open() {
            this._readFromDOM();
            this.statusMessage = '';
            this.showModal = true;
            // Don't reset `running` here — if an update is in flight the button
            // should stay disabled through the async status check.
            this._checkStatusOnce();
            this._startWatch();
        },

        _startWatch() {
            // Slow background refresh so a background auto-update that lands
            // while the modal sits open still shows up without the user having
            // to close and reopen. _checkStatusOnce refreshes content and will
            // promote to the 2s fast poll if it catches a running update.
            this._stopWatch();
            this._watchHandle = setInterval(() => {
                this._checkStatusOnce();
            }, 15000);
        },

        _stopWatch() {
            if (this._watchHandle) {
                clearInterval(this._watchHandle);
                this._watchHandle = null;
            }
        },

        async _checkStatusOnce() {
            if (!this.sessionId) {
                this.running = false;
                return;
            }
            try {
                const response = await fetch(`${this._statusUrl}?session_id=${encodeURIComponent(this.sessionId)}`);
                if (!response.ok) {
                    this.running = false;
                    return;
                }
                const data = await response.json();

                // Refresh content from the server: a background auto-update
                // may have written new memory while the modal was closed or
                // on a different page. Also syncs the DOM data source so a
                // subsequent reopen reflects the latest without refetching.
                if (typeof data.memory === 'string') {
                    this.content = data.memory;
                    this.updatedAt = data.updated_at || '';
                    const source = document.getElementById('thread-memory-data');
                    if (source) {
                        source.dataset.memory = this.content;
                        source.dataset.updatedAt = this.updatedAt;
                    }
                }

                if (data.state === 'running') {
                    this.running = true;
                    this.statusMessage = 'Summarizing thread...';
                    this.statusType = 'info';
                    this._startPolling();
                } else {
                    this.running = false;
                }
            } catch (e) {
                this.running = false;
            }
        },

        async updateNow() {
            if (!this.sessionId) {
                this.statusMessage = 'No active session.';
                this.statusType = 'error';
                return;
            }

            this.running = true;
            this.statusMessage = 'Summarizing thread...';
            this.statusType = 'info';

            const body = new URLSearchParams();
            body.append('session_id', this.sessionId);

            try {
                const response = await fetch(this._updateUrl, {
                    method: 'POST',
                    headers: {
                        'X-CSRFToken': getCsrfToken(),
                        'Content-Type': 'application/x-www-form-urlencoded',
                    },
                    body: body.toString(),
                });

                if (response.status === 202) {
                    this._startPolling();
                } else if (response.status === 409) {
                    this.statusMessage = 'An update is already running for this thread.';
                    this.statusType = 'info';
                    this._startPolling();
                } else {
                    const data = await response.json().catch(() => ({}));
                    this.statusMessage = data.error || `Update failed (${response.status}).`;
                    this.statusType = 'error';
                    this.running = false;
                }
            } catch (e) {
                this.statusMessage = `Update failed: ${e.message}`;
                this.statusType = 'error';
                this.running = false;
            }
        },

        _startPolling() {
            this._stopPolling();
            const maxDuration = 10 * 60 * 1000;
            const startTime = Date.now();

            this._pollHandle = setInterval(async () => {
                if (Date.now() - startTime > maxDuration) {
                    this._stopPolling();
                    this.statusMessage = 'Update timed out.';
                    this.statusType = 'error';
                    this.running = false;
                    return;
                }

                try {
                    const response = await fetch(`${this._statusUrl}?session_id=${encodeURIComponent(this.sessionId)}`);
                    if (!response.ok) return;
                    const data = await response.json();

                    if (data.state === 'completed') {
                        this._stopPolling();
                        this.running = false;
                        // Refresh displayed content from the just-completed update
                        this.content = data.memory || '';
                        this.updatedAt = data.updated_at || '';
                        if (data.error) {
                            this.statusMessage = data.error;
                            this.statusType = 'info';
                        } else {
                            this.statusMessage = 'Thread memory updated.';
                            this.statusType = 'success';
                        }
                        // Sync the DOM data source so a re-open reflects the new value
                        const source = document.getElementById('thread-memory-data');
                        if (source) {
                            source.dataset.memory = this.content;
                            source.dataset.updatedAt = this.updatedAt;
                        }
                    } else if (data.state === 'failed') {
                        this._stopPolling();
                        this.running = false;
                        this.statusMessage = data.error || 'Update failed.';
                        this.statusType = 'error';
                    }
                    // state === 'running': leave existing content visible, keep polling
                } catch (e) {
                    // Transient fetch error — keep polling
                }
            }, 2000);
        },

        _stopPolling() {
            if (this._pollHandle) {
                clearInterval(this._pollHandle);
                this._pollHandle = null;
            }
        },

        _applySettingsResponse(data) {
            const eff = (data && data.effective) || {};
            if (typeof eff.interval_minutes === 'number') this.intervalMinutes = eff.interval_minutes;
            if (typeof eff.message_floor === 'number') this.messageFloor = eff.message_floor;
            if (typeof eff.size_limit === 'number') this.sizeLimit = eff.size_limit;
            this.hasOverride = !!(data && data.has_override);
            this.settingsDirty = false;

            // Keep the DOM data source in sync so a reopen reflects new state
            const source = document.getElementById('thread-memory-data');
            if (source) {
                source.dataset.intervalMinutes = String(this.intervalMinutes);
                source.dataset.messageFloor = String(this.messageFloor);
                source.dataset.sizeLimit = String(this.sizeLimit);
                source.dataset.hasOverride = this.hasOverride ? 'true' : 'false';
            }
        },

        async saveSettings() {
            if (!this.sessionId) {
                showToast('No active session.', 'error');
                return;
            }
            this.settingsSaving = true;

            // Coerce first so empty/NaN inputs don't reach the server as "NaN".
            // Assign back so the visible field matches what we send.
            this.intervalMinutes = this.clampInterval(this.intervalMinutes);
            this.messageFloor = this.clampMessageFloor(this.messageFloor);
            this.sizeLimit = this.clampSizeLimit(this.sizeLimit);

            const body = new URLSearchParams();
            body.append('session_id', this.sessionId);
            body.append('interval_minutes', String(this.intervalMinutes));
            body.append('message_floor', String(this.messageFloor));
            body.append('size_limit', String(this.sizeLimit));

            try {
                const response = await fetch(this._settingsSaveUrl, {
                    method: 'POST',
                    headers: {
                        'X-CSRFToken': getCsrfToken(),
                        'Content-Type': 'application/x-www-form-urlencoded',
                    },
                    body: body.toString(),
                });
                if (response.ok) {
                    const data = await response.json();
                    this._applySettingsResponse(data);
                    showToast('Settings saved.', 'success');
                } else {
                    const data = await response.json().catch(() => ({}));
                    showToast(data.error || `Save failed (${response.status}).`, 'error');
                }
            } catch (e) {
                showToast(`Save failed: ${e.message}`, 'error');
            } finally {
                this.settingsSaving = false;
            }
        },

        async resetSettings() {
            if (!this.sessionId) return;
            this.settingsSaving = true;

            const body = new URLSearchParams();
            body.append('session_id', this.sessionId);

            try {
                const response = await fetch(this._settingsResetUrl, {
                    method: 'POST',
                    headers: {
                        'X-CSRFToken': getCsrfToken(),
                        'Content-Type': 'application/x-www-form-urlencoded',
                    },
                    body: body.toString(),
                });
                if (response.ok) {
                    const data = await response.json();
                    this._applySettingsResponse(data);
                    showToast('Reset to persona defaults.', 'success');
                } else {
                    const data = await response.json().catch(() => ({}));
                    showToast(data.error || `Reset failed (${response.status}).`, 'error');
                }
            } catch (e) {
                showToast(`Reset failed: ${e.message}`, 'error');
            } finally {
                this.settingsSaving = false;
            }
        }
    };
}

// =============================================================================
// Scenario Modal Component
// =============================================================================

function scenarioModal() {
    return {
        showModal: false,
        sessionId: '',
        content: '',
        saving: false,
        _saveUrl: '',

        init() {
            this._saveUrl = this.$el.dataset.saveUrl;
            window.addEventListener('open-scenario-modal', () => this.open());
        },

        open() {
            // Read from the hidden data div (updated on every chat_main.html swap)
            const source = document.getElementById('scenario-data');
            this.sessionId = source ? source.dataset.sessionId : '';
            this.content = source ? source.dataset.scenario : '';
            this.showModal = true;
        },

        async save() {
            // Home mode: no session exists yet. Just propagate the content
            // back to the home page component and close. Submission will
            // carry the scenario along as a form field.
            if (!this.sessionId) {
                window.dispatchEvent(new CustomEvent('home-scenario-saved', {
                    detail: { content: this.content },
                }));
                this.showModal = false;
                return;
            }

            this.saving = true;

            const body = new URLSearchParams();
            body.append('session_id', this.sessionId);
            body.append('scenario', this.content);

            try {
                const response = await fetch(this._saveUrl, {
                    method: 'POST',
                    headers: {
                        'X-CSRFToken': getCsrfToken(),
                        'Content-Type': 'application/x-www-form-urlencoded',
                    },
                    body: body.toString(),
                });

                if (response.ok) {
                    // Update the in-page data source so a reopen reflects the new value
                    const source = document.getElementById('scenario-data');
                    if (source) source.dataset.scenario = this.content;
                    this.showModal = false;
                    showToast('Scenario saved.', 'success');
                } else {
                    showToast(`Save failed (${response.status}).`, 'error');
                }
            } catch (e) {
                showToast(`Save failed: ${e.message}`, 'error');
            } finally {
                this.saving = false;
            }
        }
    };
}

// =============================================================================
// Edit Persona Modal Component
// =============================================================================

function editPersonaModal() {
    return {
        showModal: false,
        isNew: false,         // true for new persona, false for edit
        isAssistant: false,   // true if editing the "assistant" persona (cannot rename)
        persona: '',          // Original folder name (empty for new)
        displayName: '',      // User-editable display name
        content: '',
        createUrl: '',
        saveUrl: '',

        init() {
            this.createUrl = this.$el.dataset.createUrl || '/settings/create-persona/';
            this.saveUrl = this.$el.dataset.saveUrl || '/settings/save-persona/';

            window.addEventListener('open-new-persona-modal', () => {
                this.openNew();
            });
            window.addEventListener('open-edit-persona-modal', () => {
                const persona = document.querySelector('[name="persona"]')?.value || '';
                // Read `.content.textContent` (decoded text), not `.innerHTML` (serialized
                // HTML). Django already entity-escaped the raw text when rendering the
                // template; innerHTML would return the escaped form, which then re-enters
                // the save loop and gains another layer of escaping on every round-trip.
                const contentTemplate = document.getElementById('persona-raw-content');
                const content = contentTemplate ? contentTemplate.content.textContent : '';
                this.openEdit(persona, content);
            });
        },

        openNew() {
            this.isNew = true;
            this.isAssistant = false;
            this.persona = '';
            this.displayName = '';
            this.content = '';
            this.showModal = true;
        },

        openEdit(persona, content) {
            this.isNew = false;
            this.isAssistant = (persona === 'assistant');
            this.persona = persona;
            this.displayName = toDisplayName(persona);
            this.content = content;
            this.showModal = true;
        },

        async savePersona() {
            // Convert display name to folder name format
            const newFolderName = toFolderName(this.displayName);

            const url = this.isNew ? this.createUrl : this.saveUrl;
            // Don't send new_name for assistant persona (cannot be renamed)
            const body = this.isNew
                ? `name=${encodeURIComponent(newFolderName)}&content=${encodeURIComponent(this.content)}`
                : this.isAssistant
                    ? `persona=${encodeURIComponent(this.persona)}&content=${encodeURIComponent(this.content)}`
                    : `persona=${encodeURIComponent(this.persona)}&new_name=${encodeURIComponent(newFolderName)}&content=${encodeURIComponent(this.content)}`;

            // Close immediately. The fetch + page refresh happen in the background
            // so the click feels instant; the server-rendered success toast fires
            // when the refreshed partial swaps in.
            this.showModal = false;

            try {
                const response = await fetch(url, {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/x-www-form-urlencoded',
                        'X-CSRFToken': getCsrfToken(),
                        'HX-Request': 'true'
                    },
                    body: body
                });
                const html = await response.text();
                if (response.ok) {
                    const mainContent = document.getElementById('main-content');
                    mainContent.innerHTML = html;
                    htmx.process(mainContent);
                } else {
                    showToast('Failed to save persona.', 'error');
                }
            } catch (e) {
                console.error('Failed to save persona:', e);
                showToast('Failed to save persona.', 'error');
            }
        }
    };
}

// =============================================================================
// Edit Persona Model Modal Component
// =============================================================================

function editPersonaModelModal() {
    return {
        showModal: false,
        persona: '',
        displayName: '',
        currentModel: '',
        selectedModel: '',
        modelsLoaded: false,
        loading: false,
        loadError: '',
        defaultModel: '',
        _modelItems: [],
        saving: false,
        modelsUrl: '',
        saveUrl: '',

        init() {
            this.modelsUrl = this.$el.dataset.modelsUrl || '/settings/available-models/';
            this.saveUrl = this.$el.dataset.saveUrl || '/settings/save-persona-model/';

            window.addEventListener('open-edit-model-modal', () => {
                const personaData = document.getElementById('persona-data');
                const persona = personaData ? personaData.dataset.selectedId : '';
                const personaModel = personaData ? personaData.dataset.personaModel : '';
                const defaultModel = personaData ? personaData.dataset.defaultModel : '';
                this.open(persona, personaModel, defaultModel);
            });
        },

        async loadModels() {
            if (this.modelsLoaded || this.loading) return;

            this.loading = true;
            this.loadError = '';

            try {
                const response = await fetch(this.modelsUrl);
                const data = await response.json();

                if (response.ok && data.models) {
                    this.modelsLoaded = true;
                    // Convert to {id, label} format for selectDropdown
                    this._modelItems = data.models.map(m => ({ id: m.id, label: m.display }));
                } else {
                    this.loadError = data.error || 'Failed to load models';
                }
            } catch (e) {
                this.loadError = 'Failed to fetch models. Please try again.';
            } finally {
                this.loading = false;
            }
        },

        /** Called by template: returns items for the dropdown */
        get modelItems() {
            return this._modelItems || [];
        },

        clearModel() {
            this.selectedModel = '';
        },

        onModelSelect(detail) {
            this.selectedModel = detail.id;
        },

        open(persona, personaModel, defaultModel) {
            this.persona = persona;
            this.displayName = toDisplayName(persona);
            this.currentModel = personaModel;
            this.selectedModel = personaModel;
            this.defaultModel = defaultModel;
            this.loadError = '';
            this.showModal = true;
            this._modelItems = [];
            this.modelsLoaded = false;

            // Fetch models lazily when modal opens
            this.loadModels();
        },

        async saveModel() {
            this.saving = true;

            const csrfToken = getCsrfToken();

            try {
                const formData = new FormData();
                formData.append('persona', this.persona);
                formData.append('model', this.selectedModel);

                const response = await fetch(this.saveUrl, {
                    method: 'POST',
                    headers: { 'X-CSRFToken': csrfToken },
                    body: formData
                });
                const data = await response.json();

                if (data.success) {
                    this.currentModel = data.model || '';
                    showToast('Model updated.', 'success');

                    // Refresh persona page to show updated model
                    setTimeout(() => {
                        this.showModal = false;
                        const personaUrl = this.$el.dataset.personaSettingsUrl || '/persona/';
                        htmx.ajax('GET', personaUrl + '?preview=' + this.persona, {target: '#main-content', swap: 'innerHTML'});
                    }, 1000);
                } else {
                    showToast(data.error || 'Failed to save model.', 'error');
                }
            } catch (e) {
                showToast('Failed to save model. Please try again.', 'error');
            } finally {
                this.saving = false;
            }
        }
    };
}

// =============================================================================
// Context Files Modal Component
// =============================================================================

function contextFilesModal() {
    return {
        showModal: false,
        activeTab: 'uploaded',
        isDragging: false,
        files: [],
        persona: '',
        editModal: {
            show: false,
            filename: '',
            content: '',
            readOnly: false
        },
        // Local directory state
        localDirectories: [],
        newDirPath: '',
        // Directory browser state
        dirBrowser: {
            show: false,
            current: '',
            parent: null,
            dirs: [],
            hasContextFiles: false,
            contextFiles: [],
            showHidden: false,
            loading: false
        },

        // Config read from data attributes
        uploadUrl: '',
        toggleUrl: '',
        deleteUrl: '',
        contentUrl: '',
        saveUrl: '',
        badgeSelector: '',

        init() {
            this.uploadUrl = this.$el.dataset.uploadUrl;
            this.toggleUrl = this.$el.dataset.toggleUrl;
            this.deleteUrl = this.$el.dataset.deleteUrl;
            this.contentUrl = this.$el.dataset.contentUrl;
            this.saveUrl = this.$el.dataset.saveUrl;
            this.badgeSelector = this.$el.dataset.badgeSelector || '';
            this._filesSourceId = this.$el.dataset.filesSource || '';

            // Listen for open events
            const eventName = this.$el.dataset.openEvent;
            if (eventName) {
                window.addEventListener(eventName, () => {
                    this.loadFiles();
                    this.showModal = true;
                });
            }

            this.loadFiles();
        },

        _getSourceEl() {
            return this._filesSourceId ? document.getElementById(this._filesSourceId) : null;
        },

        _syncFilesToSource() {
            const el = this._getSourceEl();
            if (el) el.dataset.files = JSON.stringify(this.files);
        },

        _syncDirsToSource() {
            const el = this._getSourceEl();
            if (el) el.dataset.localDirs = JSON.stringify(this.localDirectories);
        },

        loadFiles() {
            const sourceEl = this._getSourceEl();
            if (sourceEl) {
                try { this.files = JSON.parse(sourceEl.dataset.files || '[]'); } catch (e) { this.files = []; }
                this.persona = sourceEl.dataset.persona || '';
                try { this.localDirectories = JSON.parse(sourceEl.dataset.localDirs || '[]'); } catch (e) { this.localDirectories = []; }
            } else {
                this.files = [];
                this.localDirectories = [];
            }
        },

        _appendPersona(formData) {
            if (this.persona) formData.append('persona', this.persona);
        },

        _personaQuery() {
            return this.persona ? `persona=${encodeURIComponent(this.persona)}&` : '';
        },

        _csrf() {
            return getCsrfToken();
        },

        handleDrop(event) {
            this.isDragging = false;
            this.uploadFiles(event.dataTransfer.files);
        },

        handleFileSelect(event) {
            this.uploadFiles(event.target.files);
            event.target.value = '';
        },

        async uploadFiles(fileList) {
            for (const file of fileList) {
                if (!file.name.endsWith('.md') && !file.name.endsWith('.txt')) {
                    this.showStatus(`${file.name}: Invalid file type`, 'error');
                    continue;
                }

                const formData = new FormData();
                formData.append('file', file);
                this._appendPersona(formData);
                formData.append('csrfmiddlewaretoken', this._csrf());

                try {
                    const response = await fetch(this.uploadUrl, {
                        method: 'POST',
                        body: formData,
                        headers: { 'X-Requested-With': 'XMLHttpRequest' }
                    });

                    if (response.ok) {
                        const data = await response.json();
                        this.files = data.files;
                        this._syncFilesToSource();
                        this.showStatus(`Uploaded ${file.name}`, 'success');
                        this.updateBadge();
                    } else {
                        this.showStatus(`Failed to upload ${file.name}`, 'error');
                    }
                } catch (err) {
                    this.showStatus(`Error uploading ${file.name}`, 'error');
                }
            }
        },

        async toggleFile(filename) {
            const formData = new FormData();
            formData.append('filename', filename);
            this._appendPersona(formData);
            formData.append('csrfmiddlewaretoken', this._csrf());

            const response = await fetch(this.toggleUrl, {
                method: 'POST',
                body: formData,
                headers: { 'X-Requested-With': 'XMLHttpRequest' }
            });

            if (response.ok) {
                const data = await response.json();
                this.files = data.files;
                this._syncFilesToSource();
                this.updateBadge();
            }
        },

        async deleteFile(filename) {
            if (!await confirmDialog({
                title: 'Delete File?',
                message: `Remove "${filename}" from context?`,
                confirmText: 'Delete',
            })) return;

            const formData = new FormData();
            formData.append('filename', filename);
            this._appendPersona(formData);
            formData.append('csrfmiddlewaretoken', this._csrf());

            const response = await fetch(this.deleteUrl, {
                method: 'POST',
                body: formData,
                headers: { 'X-Requested-With': 'XMLHttpRequest' }
            });

            if (response.ok) {
                const data = await response.json();
                this.files = data.files;
                this._syncFilesToSource();
                this.showStatus(`Deleted ${filename}`, 'success');
                this.updateBadge();
            }
        },

        showStatus(message, type) {
            showToast(message, type);
        },

        updateBadge() {
            if (!this.badgeSelector) return;
            const enabledUploadedCount = this.files.filter(f => f.enabled).length;
            const enabledLocalCount = this.localDirectories.reduce((sum, dir) =>
                sum + dir.files.filter(f => f.enabled).length, 0);
            const totalCount = enabledUploadedCount + enabledLocalCount;
            const badge = document.querySelector(this.badgeSelector);
            if (badge) {
                badge.textContent = totalCount;
                badge.style.display = totalCount > 0 ? 'inline' : 'none';
            }
        },

        async openEditFile(filename, readOnly) {
            this.editModal.filename = filename;
            this.editModal.readOnly = readOnly || false;

            let url = `${this.contentUrl}?${this._personaQuery()}filename=${encodeURIComponent(filename)}`;

            const response = await fetch(url, {
                headers: { 'X-Requested-With': 'XMLHttpRequest' }
            });

            if (response.ok) {
                const data = await response.json();
                this.editModal.content = data.content;
                this.editModal.show = true;
            } else {
                this.showStatus(`Failed to load ${filename}`, 'error');
            }
        },

        async saveEditFile() {
            if (this.editModal.readOnly) return;

            const formData = new FormData();
            formData.append('filename', this.editModal.filename);
            formData.append('content', this.editModal.content);
            this._appendPersona(formData);
            formData.append('csrfmiddlewaretoken', this._csrf());

            const response = await fetch(this.saveUrl, {
                method: 'POST',
                body: formData,
                headers: { 'X-Requested-With': 'XMLHttpRequest' }
            });

            if (response.ok) {
                showToast('File saved.', 'success');
                setTimeout(() => { this.editModal.show = false; }, 1000);
            } else {
                showToast('Failed to save file.', 'error');
            }
        },

        // Directory browser methods
        async openBrowser() {
            this.dirBrowser.show = true;
            await this.browseTo('');
        },

        async browseTo(path) {
            this.dirBrowser.loading = true;
            try {
                const params = new URLSearchParams({ path: path || '' });
                if (this.dirBrowser.showHidden) params.append('show_hidden', '1');
                const response = await fetch(`/context/local/browse/?${params}`, {
                    headers: { 'X-Requested-With': 'XMLHttpRequest' }
                });
                if (response.ok) {
                    const data = await response.json();
                    this.dirBrowser.current = data.current;
                    this.dirBrowser.parent = data.parent;
                    this.dirBrowser.dirs = data.dirs;
                    this.dirBrowser.hasContextFiles = data.has_context_files;
                    this.dirBrowser.contextFiles = data.context_files || [];
                    if (data.error) showToast(data.error, 'error');
                }
            } catch (err) {
                showToast('Failed to browse directory.', 'error');
            }
            this.dirBrowser.loading = false;
        },

        async toggleHidden() {
            this.dirBrowser.showHidden = !this.dirBrowser.showHidden;
            await this.browseTo(this.dirBrowser.current);
        },

        selectBrowserDir() {
            this.newDirPath = this.dirBrowser.current;
            this.dirBrowser.show = false;
            this.addDirectory();
        },

        // Local directory methods
        async addDirectory() {
            if (!this.newDirPath.trim()) return;

            const formData = new FormData();
            formData.append('dir_path', this.newDirPath.trim());
            this._appendPersona(formData);
            formData.append('csrfmiddlewaretoken', this._csrf());

            try {
                const response = await fetch('/context/local/add/', {
                    method: 'POST',
                    body: formData,
                    headers: { 'X-Requested-With': 'XMLHttpRequest' }
                });

                const data = await response.json();
                if (response.ok) {
                    this.localDirectories = data.directories;
                    this._syncDirsToSource();
                    this.newDirPath = '';
                    this.showStatus('Directory added', 'success');
                    this.updateBadge();
                } else {
                    this.showStatus(data.error || 'Failed to add directory', 'error');
                }
            } catch (err) {
                this.showStatus('Error adding directory', 'error');
            }
        },

        async removeDirectory(dirPath) {
            if (!await confirmDialog({
                title: 'Remove Directory?',
                message: 'This directory will no longer contribute context files.',
                confirmText: 'Remove',
            })) return;

            const formData = new FormData();
            formData.append('dir_path', dirPath);
            this._appendPersona(formData);
            formData.append('csrfmiddlewaretoken', this._csrf());

            try {
                const response = await fetch('/context/local/remove/', {
                    method: 'POST',
                    body: formData,
                    headers: { 'X-Requested-With': 'XMLHttpRequest' }
                });

                if (response.ok) {
                    const data = await response.json();
                    this.localDirectories = data.directories;
                    this._syncDirsToSource();
                    this.showStatus('Directory removed', 'success');
                    this.updateBadge();
                }
            } catch (err) {
                this.showStatus('Error removing directory', 'error');
            }
        },

        async toggleLocalFile(dirPath, filename) {
            const formData = new FormData();
            formData.append('dir_path', dirPath);
            formData.append('filename', filename);
            this._appendPersona(formData);
            formData.append('csrfmiddlewaretoken', this._csrf());

            try {
                const response = await fetch('/context/local/toggle/', {
                    method: 'POST',
                    body: formData,
                    headers: { 'X-Requested-With': 'XMLHttpRequest' }
                });

                if (response.ok) {
                    const data = await response.json();
                    this.localDirectories = data.directories;
                    this._syncDirsToSource();
                    this.updateBadge();
                }
            } catch (err) {
                this.showStatus('Error toggling file', 'error');
            }
        },

        async viewLocalFile(dirPath, filename) {
            this.editModal.filename = filename;
            this.editModal.readOnly = true;

            try {
                const response = await fetch(`/context/local/content/?${this._personaQuery()}dir_path=${encodeURIComponent(dirPath)}&filename=${encodeURIComponent(filename)}`, {
                    headers: { 'X-Requested-With': 'XMLHttpRequest' }
                });

                if (response.ok) {
                    const data = await response.json();
                    this.editModal.content = data.content;
                    this.editModal.show = true;
                } else {
                    this.showStatus(`Failed to load ${filename}`, 'error');
                }
            } catch (err) {
                this.showStatus(`Error loading ${filename}`, 'error');
            }
        },

        async refreshDirectory(dirPath) {
            const formData = new FormData();
            formData.append('dir_path', dirPath);
            this._appendPersona(formData);
            formData.append('csrfmiddlewaretoken', this._csrf());

            try {
                const response = await fetch('/context/local/refresh/', {
                    method: 'POST',
                    body: formData,
                    headers: { 'X-Requested-With': 'XMLHttpRequest' }
                });

                if (response.ok) {
                    const data = await response.json();
                    this.localDirectories = data.directories;
                    this._syncDirsToSource();
                    this.showStatus('Directory refreshed', 'success');
                    this.updateBadge();
                }
            } catch (err) {
                this.showStatus('Error refreshing directory', 'error');
            }
        }
    };
}

// =============================================================================
// Provider Model Settings Component
// =============================================================================

function providerModelSettings() {
    return {
        // Properties with defaults (populated in init from data-* attributes)
        currentProvider: '',
        currentModel: '',
        hasExistingKey: false,
        providers: [],
        selectedProvider: '',
        selectedProviderName: '',
        apiKey: '',
        apiKeyModified: false,
        apiKeyValid: false,
        apiKeyError: '',
        validating: false,
        selectedModel: '',
        saving: false,

        // Items for nested selectDropdown components
        providerItems: [],
        _modelItems: [],

        // URLs (populated from data attributes)
        validateUrl: '',
        saveUrl: '',
        csrfToken: '',

        init() {
            const el = this.$el;
            this.currentProvider = el.dataset.provider || '';
            this.currentModel = el.dataset.model || '';
            this.hasExistingKey = el.dataset.hasExistingKey === 'true';
            this.selectedProvider = el.dataset.provider || 'openrouter';
            this.selectedProviderName = el.dataset.providerName || '';
            this.selectedModel = el.dataset.model || '';
            this.apiKeyValid = el.dataset.hasExistingKey === 'true';
            this.validateUrl = el.dataset.validateUrl || '';
            this.saveUrl = el.dataset.saveUrl || '';
            this.csrfToken = el.dataset.csrfToken || '';

            // Parse providers from JSON
            try {
                this.providers = JSON.parse(el.dataset.providers || '[]');
            } catch (e) {
                this.providers = [];
            }

            // Convert providers to {id, label} for dropdown
            this.providerItems = this.providers.map(p => ({ id: p.id, label: p.name }));

            // Load models if we have an existing key
            if (this.hasExistingKey) {
                this.loadExistingModels();
            }
        },

        get currentProviderData() {
            return this.providers.find(p => p.id === this.selectedProvider);
        },

        get showModelPicker() {
            return this.apiKeyValid || this.hasExistingKey;
        },

        get modelItems() {
            return this._modelItems;
        },

        get canSave() {
            const hasValidKey = this.apiKeyValid || (this.hasExistingKey && !this.apiKeyModified);
            return hasValidKey && this.selectedModel;
        },

        onProviderSelect(detail) {
            const provider = this.providers.find(p => p.id === detail.id);
            if (!provider) return;
            this.selectedProvider = provider.id;
            this.selectedProviderName = provider.name;
            if (provider.id !== this.currentProvider) {
                this.apiKey = '';
                this.apiKeyModified = true;
                this.apiKeyValid = false;
                this._modelItems = [];
                this.selectedModel = '';
            }
        },

        onModelSelect(detail) {
            this.selectedModel = detail.id;
        },

        onApiKeyChange() {
            this.apiKeyModified = true;
            this.apiKeyValid = false;
            this.apiKeyError = '';
        },

        async loadExistingModels() {
            try {
                const formData = new FormData();
                formData.append('provider', this.selectedProvider);
                formData.append('use_existing', 'true');

                const response = await fetch(this.validateUrl, {
                    method: 'POST',
                    headers: { 'X-CSRFToken': this.csrfToken },
                    body: formData
                });
                const data = await response.json();
                if (data.valid && data.models) {
                    this._modelItems = data.models.map(m => ({ id: m.id, label: m.display }));
                }
            } catch (e) {
                console.error('Failed to load models:', e);
                showToast('Failed to load models. Please try again.', 'error');
            }
        },

        async validateApiKey() {
            this.validating = true;
            this.apiKeyError = '';

            try {
                const formData = new FormData();
                formData.append('provider', this.selectedProvider);
                formData.append('api_key', this.apiKey);

                const response = await fetch(this.validateUrl, {
                    method: 'POST',
                    headers: { 'X-CSRFToken': this.csrfToken },
                    body: formData
                });
                const data = await response.json();

                if (data.valid) {
                    this.apiKeyValid = true;
                    this._modelItems = (data.models || []).map(m => ({ id: m.id, label: m.display }));
                    this.selectedModel = '';
                } else {
                    this.apiKeyError = data.error || 'Invalid API key';
                }
            } catch (e) {
                this.apiKeyError = 'Validation failed. Please try again.';
            } finally {
                this.validating = false;
            }
        },

        async saveProviderModel() {
            this.saving = true;

            try {
                const formData = new FormData();
                formData.append('provider', this.selectedProvider);
                formData.append('model', this.selectedModel);

                if (this.apiKeyModified && this.apiKey) {
                    formData.append('api_key', this.apiKey);
                } else {
                    formData.append('keep_existing_key', 'true');
                }

                const response = await fetch(this.saveUrl, {
                    method: 'POST',
                    headers: { 'X-CSRFToken': this.csrfToken },
                    body: formData
                });
                const data = await response.json();

                if (data.success) {
                    this.currentProvider = data.provider;
                    this.currentModel = data.model;
                    this.hasExistingKey = true;
                    this.apiKeyModified = false;
                    showToast('Provider and model updated.', 'success');
                } else {
                    showToast(data.error || 'Failed to save settings.', 'error');
                }
            } catch (e) {
                showToast('Failed to save settings. Please try again.', 'error');
            } finally {
                this.saving = false;
            }
        }
    };
}

// =============================================================================
// Home Persona Picker Component
// =============================================================================

function homePersonaPicker() {
    return {
        selectedPersona: '',
        selectedMode: 'chatbot',
        scenarioContent: '',
        personaItems: [],
        modeItems: [
            { id: 'chatbot', label: 'Chatbot' },
            { id: 'roleplay', label: 'Roleplay' },
        ],
        personaModels: {},
        personaModes: {},
        defaultModel: '',

        get currentModel() {
            return this.personaModels[this.selectedPersona] || this.defaultModel;
        },

        get currentModelDisplay() {
            const model = this.currentModel;
            return model.includes('/') ? model.split('/').pop() : model;
        },

        onPersonaSelect(detail) {
            this.selectedPersona = detail.id;
            // Honor the persona's default mode when switching personas
            const defaultMode = this.personaModes[detail.id];
            if (defaultMode === 'chatbot' || defaultMode === 'roleplay') {
                this.selectedMode = defaultMode;
            }
        },

        onModeSelect(detail) {
            if (detail.id === 'chatbot' || detail.id === 'roleplay') {
                this.selectedMode = detail.id;
            }
        },

        init() {
            const el = this.$el;

            // Parse personas from data attribute and convert to {id, label}
            try {
                const personas = JSON.parse(el.dataset.personas || '[]');
                this.personaItems = personas.map(p => ({ id: p.id, label: p.display }));
            } catch (e) {
                this.personaItems = [];
            }

            this.selectedPersona = el.dataset.defaultPersona || '';

            // Load data from hidden element (survives HTMX swaps)
            const dataEl = document.getElementById('home-data');
            if (dataEl) {
                try {
                    this.personaModels = JSON.parse(dataEl.dataset.personaModels || '{}');
                } catch (e) {
                    this.personaModels = {};
                }
                try {
                    this.personaModes = JSON.parse(dataEl.dataset.personaModes || '{}');
                } catch (e) {
                    this.personaModes = {};
                }
                this.defaultModel = dataEl.dataset.defaultModel || '';
            }

            // Seed selectedMode from the default persona's configured default_mode
            const initialMode = this.personaModes[this.selectedPersona];
            if (initialMode === 'chatbot' || initialMode === 'roleplay') {
                this.selectedMode = initialMode;
            }

            // Restore any saved home-page scenario
            this.scenarioContent = localStorage.getItem('home-chat-scenario') || '';

            // Scenario modal reports back via this event (home mode path)
            window.addEventListener('home-scenario-saved', (e) => {
                this.scenarioContent = e.detail.content || '';
                if (this.scenarioContent) {
                    localStorage.setItem('home-chat-scenario', this.scenarioContent);
                } else {
                    localStorage.removeItem('home-chat-scenario');
                }
            });

            // Clear hook — called from the form's before-request handler
            window.addEventListener('home-scenario-cleared', () => {
                this.scenarioContent = '';
                localStorage.removeItem('home-chat-scenario');
            });

            // Set timezone
            setTimezoneInput();
        }
    };
}

// =============================================================================
// Memory Persona Picker Component
// =============================================================================

function memoryPersonaPicker() {
    return {
        selectedPersona: '',
        personaItems: [],
        memoryUrl: '',

        onPersonaSelect(detail) {
            this.selectedPersona = detail.id;
            htmx.ajax('GET', this.memoryUrl + '?persona=' + detail.id, {target: '#main-content', swap: 'innerHTML'});
        },

        init() {
            const el = this.$el;

            try {
                const personas = JSON.parse(el.dataset.personas || '[]');
                this.personaItems = personas.map(p => ({ id: p.id, label: p.display }));
            } catch (e) {
                this.personaItems = [];
            }

            this.selectedPersona = el.dataset.selectedPersona || '';
            this.memoryUrl = el.dataset.memoryUrl || '/memory/';
        }
    };
}

// =============================================================================
// Persona Settings Picker Component
// =============================================================================

function personaSettingsPicker() {
    return {
        selectedPersona: '',
        personaItems: [],
        settingsUrl: '',

        onPersonaSelect(detail) {
            this.selectedPersona = detail.id;
            // Trigger HTMX preview (preserve scroll position)
            const scrollContainer = document.querySelector('#main-content .overflow-y-auto');
            const scrollPos = scrollContainer ? scrollContainer.scrollTop : 0;
            htmx.ajax('GET', this.settingsUrl + '?preview=' + detail.id, {target: '#main-content', swap: 'innerHTML'}).then(() => {
                if (scrollContainer) {
                    const newScrollContainer = document.querySelector('#main-content .overflow-y-auto');
                    if (newScrollContainer) newScrollContainer.scrollTop = scrollPos;
                }
            });
        },

        init() {
            const el = this.$el;

            // Parse personas from data attribute and convert to {id, label}
            try {
                const personas = JSON.parse(el.dataset.personas || '[]');
                this.personaItems = personas.map(p => ({ id: p.id, label: p.display }));
            } catch (e) {
                this.personaItems = [];
            }

            this.selectedPersona = el.dataset.selectedPersona || '';
            this.settingsUrl = el.dataset.settingsUrl || '/persona/';
        }
    };
}

// =============================================================================
// Provider Picker Component (Setup Step 1)
// =============================================================================

function providerPicker() {
    return {
        selectedId: '',
        selectedProvider: null,
        providers: [],
        providerItems: [],

        init() {
            const el = this.$el;

            // Parse providers from data attribute
            try {
                this.providers = JSON.parse(el.dataset.providers || '[]');
            } catch (e) {
                this.providers = [];
            }

            this.providerItems = this.providers.map(p => ({ id: p.id, label: p.name }));
            this.selectedId = el.dataset.selectedProvider || 'openrouter';

            // Auto-select first provider if only one
            if (this.providers.length === 1) {
                this.selectedId = this.providers[0].id;
                this.selectedProvider = this.providers[0];
            } else {
                this.selectedProvider = this.providers.find(p => p.id === this.selectedId) || null;
            }
        },

        onProviderSelect(detail) {
            const provider = this.providers.find(p => p.id === detail.id);
            if (!provider) return;
            this.selectedId = provider.id;
            this.selectedProvider = provider;
        }
    };
}

// =============================================================================
// Model Picker Component (Setup Step 2)
// =============================================================================

function modelPicker() {
    return {
        selectedId: '',
        modelItems: [],

        onModelSelect(detail) {
            this.selectedId = detail.id;
            this.updateButton();
        },

        updateButton() {
            const btn = document.getElementById('submitBtn');
            if (btn) btn.disabled = !this.selectedId;
        },

        init() {
            const el = this.$el;

            // Parse models from data attribute and convert to {id, label}
            try {
                const models = JSON.parse(el.dataset.models || '[]');
                this.modelItems = models.map(m => ({ id: m.id, label: m.display }));
            } catch (e) {
                this.modelItems = [];
            }

            this.selectedId = el.dataset.selectedModel || '';
            this.updateButton();
        }
    };
}

// =============================================================================
// Theme Picker Component (Settings Page)
// =============================================================================

/**
 * Theme picker dropdown for selecting color themes.
 * Fetches themes from backend API and saves preference to config.json.
 */
function themePicker() {
    return {
        themeItems: [],
        currentTheme: '',

        async onThemeSelect(detail) {
            this.currentTheme = detail.id;
            await loadTheme(detail.id);
            await saveThemePreference(detail.id, getTheme());
        },

        async init() {
            const themes = await getAvailableThemes();
            this.themeItems = themes.map(t => ({ id: t.id, label: t.name }));
            this.currentTheme = getColorTheme();
        }
    };
}

// =============================================================================
// Setup Theme Picker Component (Setup Wizard Step 2)
// =============================================================================

/**
 * Theme picker for the setup wizard.
 * Uses data attributes to receive theme list and initial selections.
 */
function setupThemePicker() {
    return {
        themeItems: [],
        selectedTheme: '',
        selectedMode: '',

        onThemeSelect(detail) {
            this.selectedTheme = detail.id;
            loadTheme(detail.id);
        },

        setMode(mode) {
            this.selectedMode = mode;
            setTheme(mode);
        },

        init() {
            const el = this.$el;

            // Parse themes from data attribute and convert to {id, label}
            try {
                const themes = JSON.parse(el.dataset.themes || '[]');
                this.themeItems = themes.map(t => ({ id: t.id, label: t.name }));
            } catch (e) {
                this.themeItems = [];
            }

            // Check localStorage first for user's actual preference
            this.selectedTheme = localStorage.getItem('colorTheme') || el.dataset.selectedTheme || 'liminal-salt';
            this.selectedMode = localStorage.getItem('theme') || el.dataset.selectedMode || 'dark';

            // Apply the theme to ensure UI matches
            loadTheme(this.selectedTheme);
        }
    };
}

// =============================================================================
// Theme Mode Toggle Component (Settings Page Dark/Light buttons)
// =============================================================================

/**
 * Dark/Light mode toggle buttons for the settings page.
 * Syncs with sidebar theme toggle via theme-mode-changed event.
 */
function themeModeToggle() {
    return {
        isDark: true,

        async setMode(mode) {
            this.isDark = mode === 'dark';
            setTheme(mode);
            // Save preference to backend
            await saveThemePreference(getColorTheme(), mode);
        },

        init() {
            // Get current mode from localStorage
            this.isDark = localStorage.getItem('theme') !== 'light';

            // Listen for theme mode changes from sidebar or other components
            window.addEventListener('theme-mode-changed', (e) => {
                this.isDark = e.detail.mode === 'dark';
            });
        }
    };
}

// =============================================================================
// Memory Settings Component (inline save via fetch)
// =============================================================================

function memorySettings() {
    return {
        userHistoryMaxThreads: 0,
        userHistoryMessagesPerThread: 0,
        memorySizeLimit: 8000,
        autoMemoryInterval: 0,
        autoMemoryMessageFloor: 10,
        saved: true,
        saving: false,
        _saveUrl: '',
        _persona: '',

        init() {
            const el = this.$el;
            this._saveUrl = el.dataset.saveUrl;
            this._persona = el.dataset.persona;
            this.userHistoryMaxThreads = parseInt(el.dataset.maxThreads) || 0;
            this.userHistoryMessagesPerThread = parseInt(el.dataset.messagesPerThread) || 0;
            this.memorySizeLimit = parseInt(el.dataset.sizeLimit) || 8000;
            this.autoMemoryInterval = parseInt(el.dataset.autoInterval) || 0;
            this.autoMemoryMessageFloor = parseInt(el.dataset.messageFloor) || 10;
        },

        async save() {
            this.saving = true;
            const form = new FormData();
            form.append('persona', this._persona);
            form.append('user_history_max_threads', this.userHistoryMaxThreads);
            form.append('user_history_messages_per_thread', this.userHistoryMessagesPerThread);
            form.append('memory_size_limit', this.memorySizeLimit);
            form.append('auto_memory_interval', this.autoMemoryInterval);
            form.append('auto_memory_message_floor', this.autoMemoryMessageFloor);

            try {
                const resp = await fetch(this._saveUrl, {
                    method: 'POST',
                    headers: { 'X-CSRFToken': getCsrfToken() },
                    body: form
                });
                if (resp.ok) {
                    this.saved = true;
                    showToast('Memory settings saved.', 'success');
                } else {
                    showToast('Failed to save memory settings.', 'error');
                }
            } catch (e) {
                console.error('Failed to save memory settings:', e);
                showToast('Failed to save memory settings.', 'error');
            } finally {
                this.saving = false;
            }
        }
    };
}

// =============================================================================
// Persona Thread Defaults Component (persona settings page)
// =============================================================================

function personaThreadDefaults() {
    return {
        persona: '',
        defaultMode: 'chatbot',
        intervalMinutes: 0,
        messageFloor: 4,
        sizeLimit: 4000,
        hasDefaults: false,
        dirty: false,
        saving: false,
        modeItems: [
            { id: 'chatbot', label: 'Chatbot' },
            { id: 'roleplay', label: 'Roleplay' },
        ],
        _saveUrl: '',
        _clearUrl: '',

        // Match server-side clamping (chat.views.personas.save_persona_thread_defaults)
        // so the displayed value reflects what would be saved. Applied on @change
        // so the user sees the snap before Save.
        clampInterval(v) {
            const n = Number.isFinite(v) ? v : 0;
            if (n <= 0) return 0;
            return Math.max(5, Math.min(1440, Math.round(n)));
        },
        clampMessageFloor(v) {
            const n = Number.isFinite(v) ? v : 4;
            return Math.max(1, Math.min(1000, Math.round(n)));
        },
        clampSizeLimit(v) {
            const n = Number.isFinite(v) ? v : 0;
            return Math.max(0, Math.min(100000, Math.round(n)));
        },

        init() {
            const el = this.$el;
            this.persona = el.dataset.persona || '';
            // Backend returns 'roleplay' or ''. Chatbot is the baseline so '' → chatbot.
            this.defaultMode = el.dataset.defaultMode === 'roleplay' ? 'roleplay' : 'chatbot';
            this.intervalMinutes = parseInt(el.dataset.intervalMinutes) || 0;
            this.messageFloor = parseInt(el.dataset.messageFloor) || 4;
            this.sizeLimit = parseInt(el.dataset.sizeLimit) || 4000;
            this.hasDefaults = el.dataset.hasThreadDefaults === 'true';
            this._saveUrl = el.dataset.saveUrl;
            this._clearUrl = el.dataset.clearUrl;
        },

        onModeSelect(detail) {
            const id = detail && detail.id === 'roleplay' ? 'roleplay' : 'chatbot';
            if (id === this.defaultMode) return;
            this.defaultMode = id;
            this.dirty = true;
        },

        _applyResponse(data) {
            if (!data) return;
            // Backend returns 'roleplay' or ''. Chatbot is baseline.
            const rawMode = (data.default_mode_raw === 'roleplay') ? 'roleplay' : 'chatbot';
            this.defaultMode = rawMode;
            const eff = data.effective || {};
            if (typeof eff.interval_minutes === 'number') this.intervalMinutes = eff.interval_minutes;
            if (typeof eff.message_floor === 'number') this.messageFloor = eff.message_floor;
            if (typeof eff.size_limit === 'number') this.sizeLimit = eff.size_limit;
            this.hasDefaults = !!data.has_thread_defaults;
            this.dirty = false;
        },

        async save() {
            if (!this.persona) return;
            this.saving = true;

            // Coerce first so empty/NaN inputs don't reach the server as "NaN".
            this.intervalMinutes = this.clampInterval(this.intervalMinutes);
            this.messageFloor = this.clampMessageFloor(this.messageFloor);
            this.sizeLimit = this.clampSizeLimit(this.sizeLimit);

            const body = new URLSearchParams();
            body.append('persona', this.persona);
            body.append('default_mode', this.defaultMode || '');
            body.append('interval_minutes', String(this.intervalMinutes));
            body.append('message_floor', String(this.messageFloor));
            body.append('size_limit', String(this.sizeLimit));
            try {
                const resp = await fetch(this._saveUrl, {
                    method: 'POST',
                    headers: {
                        'X-CSRFToken': getCsrfToken(),
                        'Content-Type': 'application/x-www-form-urlencoded',
                    },
                    body: body.toString(),
                });
                if (resp.ok) {
                    const data = await resp.json();
                    this._applyResponse(data);
                    showToast('Defaults saved.', 'success');
                } else {
                    const data = await resp.json().catch(() => ({}));
                    showToast(data.error || `Save failed (${resp.status}).`, 'error');
                }
            } catch (e) {
                showToast(`Save failed: ${e.message}`, 'error');
            } finally {
                this.saving = false;
            }
        },

        async clear() {
            if (!this.persona) return;
            this.saving = true;
            const body = new URLSearchParams();
            body.append('persona', this.persona);
            try {
                const resp = await fetch(this._clearUrl, {
                    method: 'POST',
                    headers: {
                        'X-CSRFToken': getCsrfToken(),
                        'Content-Type': 'application/x-www-form-urlencoded',
                    },
                    body: body.toString(),
                });
                if (resp.ok) {
                    const data = await resp.json();
                    this._applyResponse(data);
                    showToast('Persona defaults cleared.', 'success');
                } else {
                    const data = await resp.json().catch(() => ({}));
                    showToast(data.error || `Clear failed (${resp.status}).`, 'error');
                }
            } catch (e) {
                showToast(`Clear failed: ${e.message}`, 'error');
            } finally {
                this.saving = false;
            }
        }
    };
}

// =============================================================================
// Context History Limit Component (inline save via fetch)
// =============================================================================

function contextHistoryLimit() {
    return {
        value: 50,
        saved: true,
        saving: false,
        _saveUrl: '',

        init() {
            this._saveUrl = this.$el.dataset.saveUrl;
            this.value = parseInt(this.$el.dataset.currentValue) || 50;
        },

        async save() {
            this.saving = true;
            const form = new FormData();
            form.append('context_history_limit', this.value);

            try {
                const resp = await fetch(this._saveUrl, {
                    method: 'POST',
                    headers: { 'X-CSRFToken': getCsrfToken() },
                    body: form
                });
                if (resp.ok) {
                    this.saved = true;
                    showToast('Context history limit saved.', 'success');
                } else {
                    showToast('Failed to save context history limit.', 'error');
                }
            } catch (e) {
                console.error('Failed to save context history limit:', e);
                showToast('Failed to save context history limit.', 'error');
            } finally {
                this.saving = false;
            }
        }
    };
}
