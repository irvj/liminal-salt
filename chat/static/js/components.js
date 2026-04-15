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

    // Modal Components
    Alpine.data('deleteModal', deleteModal);
    Alpine.data('renameModal', renameModal);
    Alpine.data('wipeMemoryModal', wipeMemoryModal);
    Alpine.data('editPersonaModal', editPersonaModal);
    Alpine.data('deletePersonaModal', deletePersonaModal);
    Alpine.data('editPersonaModelModal', editPersonaModelModal);
    Alpine.data('contextFilesModal', contextFilesModal);

    // Page Components
    Alpine.data('sidebarState', sidebarState);
    Alpine.data('providerModelSettings', providerModelSettings);
    Alpine.data('homePersonaPicker', homePersonaPicker);
    Alpine.data('memoryPersonaPicker', memoryPersonaPicker);
    Alpine.data('personaSettingsPicker', personaSettingsPicker);
    Alpine.data('providerPicker', providerPicker);
    Alpine.data('modelPicker', modelPicker);
    Alpine.data('themePicker', themePicker);
    Alpine.data('setupThemePicker', setupThemePicker);
    Alpine.data('themeModeToggle', themeModeToggle);
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
// Delete Modal Component
// =============================================================================

function deleteModal() {
    return {
        showModal: false,
        sessionId: '',
        sessionTitle: '',

        init() {
            window.addEventListener('open-delete-modal', (e) => {
                this.open(e.detail.id, e.detail.title);
            });
        },

        open(sessionId, sessionTitle) {
            this.sessionId = sessionId;
            this.sessionTitle = sessionTitle;
            this.showModal = true;
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
// Wipe Memory Modal Component
// =============================================================================

function wipeMemoryModal() {
    return {
        showModal: false,
        wipeUrl: '',

        init() {
            this.wipeUrl = this.$el.dataset.wipeUrl || '/memory/wipe/';
            window.addEventListener('open-wipe-memory-modal', () => {
                this.open();
            });
        },

        open() {
            this.showModal = true;
        },

        confirmWipe() {
            this.showModal = false;
            const csrfToken = getCsrfToken();

            // Include current persona in the wipe request
            const personaInput = document.querySelector('input[name="persona"]');
            const persona = personaInput ? personaInput.value : '';
            const body = new URLSearchParams();
            body.append('persona', persona);

            fetch(this.wipeUrl, {
                method: 'POST',
                headers: {
                    'X-CSRFToken': csrfToken,
                    'HX-Request': 'true',
                    'Content-Type': 'application/x-www-form-urlencoded'
                },
                body: body.toString()
            }).then(response => response.text())
            .then(html => {
                const mainContent = document.getElementById('main-content');
                mainContent.innerHTML = html;
                htmx.process(mainContent);
            });
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
                const contentTemplate = document.getElementById('persona-raw-content');
                const content = contentTemplate ? contentTemplate.innerHTML : '';
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

        savePersona() {
            const csrfToken = getCsrfToken();

            // Convert display name to folder name format
            const newFolderName = toFolderName(this.displayName);

            const url = this.isNew ? this.createUrl : this.saveUrl;
            // Don't send new_name for assistant persona (cannot be renamed)
            const body = this.isNew
                ? `name=${encodeURIComponent(newFolderName)}&content=${encodeURIComponent(this.content)}`
                : this.isAssistant
                    ? `persona=${encodeURIComponent(this.persona)}&content=${encodeURIComponent(this.content)}`
                    : `persona=${encodeURIComponent(this.persona)}&new_name=${encodeURIComponent(newFolderName)}&content=${encodeURIComponent(this.content)}`;

            fetch(url, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/x-www-form-urlencoded',
                    'X-CSRFToken': csrfToken,
                    'HX-Request': 'true'
                },
                body: body
            })
            .then(response => response.text())
            .then(html => {
                const mainContent = document.getElementById('main-content');
                mainContent.innerHTML = html;
                htmx.process(mainContent);  // Re-initialize HTMX on new content
                this.showModal = false;
            });
        }
    };
}

// =============================================================================
// Delete Persona Modal Component
// =============================================================================

function deletePersonaModal() {
    return {
        showModal: false,
        persona: '',
        displayName: '',
        deleteUrl: '',

        init() {
            this.deleteUrl = this.$el.dataset.deleteUrl || '/settings/delete-persona/';
            window.addEventListener('open-delete-persona-modal', () => {
                const persona = document.querySelector('[name="persona"]')?.value || '';
                this.open(persona);
            });
        },

        open(persona) {
            this.persona = persona;
            this.displayName = toDisplayName(persona);
            this.showModal = true;
        },

        confirmDelete() {
            const csrfToken = getCsrfToken();

            fetch(this.deleteUrl, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/x-www-form-urlencoded',
                    'X-CSRFToken': csrfToken,
                    'HX-Request': 'true'
                },
                body: `persona=${encodeURIComponent(this.persona)}`
            })
            .then(response => response.text())
            .then(html => {
                const mainContent = document.getElementById('main-content');
                mainContent.innerHTML = html;
                htmx.process(mainContent);
                this.showModal = false;
            });
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
        statusMessage: '',
        statusType: '',
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
            this.statusMessage = '';
            this.loadError = '';
            this.showModal = true;
            this._modelItems = [];
            this.modelsLoaded = false;

            // Fetch models lazily when modal opens
            this.loadModels();
        },

        async saveModel() {
            this.saving = true;
            this.statusMessage = '';

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
                    this.statusMessage = 'Model updated successfully!';
                    this.statusType = 'success';
                    this.currentModel = data.model || '';

                    // Refresh persona page to show updated model
                    setTimeout(() => {
                        this.showModal = false;
                        htmx.ajax('GET', '/persona/?preview=' + this.persona, {target: '#main-content', swap: 'innerHTML'});
                    }, 1000);
                } else {
                    this.statusMessage = data.error || 'Failed to save model';
                    this.statusType = 'error';
                }
            } catch (e) {
                this.statusMessage = 'Failed to save model. Please try again.';
                this.statusType = 'error';
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
        uploadStatus: '',
        uploadStatusType: '',
        editModal: {
            show: false,
            filename: '',
            content: '',
            readOnly: false,
            status: '',
            statusType: ''
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
            loading: false,
            error: ''
        },

        // Config read from data attributes
        uploadUrl: '',
        toggleUrl: '',
        deleteUrl: '',
        contentUrl: '',
        saveUrl: '',
        dataKey: '',
        localDirsDataKey: '',
        badgeSelector: '',

        init() {
            this.uploadUrl = this.$el.dataset.uploadUrl;
            this.toggleUrl = this.$el.dataset.toggleUrl;
            this.deleteUrl = this.$el.dataset.deleteUrl;
            this.contentUrl = this.$el.dataset.contentUrl;
            this.saveUrl = this.$el.dataset.saveUrl;
            this.dataKey = this.$el.dataset.dataKey;
            this.localDirsDataKey = this.$el.dataset.localDirsDataKey || '';
            this.badgeSelector = this.$el.dataset.badgeSelector || '';

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

        loadFiles() {
            this.files = window[this.dataKey] || [];
            if (this.$el.dataset.personaKey) {
                this.persona = window[this.$el.dataset.personaKey] || '';
            }
            if (this.localDirsDataKey) {
                this.localDirectories = window[this.localDirsDataKey] || [];
            }
        },

        _appendPersona(formData) {
            if (this.persona) formData.append('persona', this.persona);
        },

        _personaQuery() {
            return this.persona ? `persona=${encodeURIComponent(this.persona)}&` : '';
        },

        _csrf() {
            return document.querySelector('[name=csrfmiddlewaretoken]').value;
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
                        window[this.dataKey] = data.files;
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
                window[this.dataKey] = data.files;
                this.updateBadge();
            }
        },

        async deleteFile(filename) {
            if (!confirm(`Delete ${filename}?`)) return;

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
                window[this.dataKey] = data.files;
                this.showStatus(`Deleted ${filename}`, 'success');
                this.updateBadge();
            }
        },

        showStatus(message, type) {
            this.uploadStatus = message;
            this.uploadStatusType = type;
            setTimeout(() => { this.uploadStatus = ''; }, 3000);
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
            this.editModal.status = '';

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
                this.editModal.status = 'Saved successfully';
                this.editModal.statusType = 'success';
                setTimeout(() => { this.editModal.show = false; }, 1000);
            } else {
                this.editModal.status = 'Failed to save';
                this.editModal.statusType = 'error';
            }
        },

        // Directory browser methods
        async openBrowser() {
            this.dirBrowser.show = true;
            this.dirBrowser.error = '';
            await this.browseTo('');
        },

        async browseTo(path) {
            this.dirBrowser.loading = true;
            this.dirBrowser.error = '';
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
                    if (data.error) this.dirBrowser.error = data.error;
                }
            } catch (err) {
                this.dirBrowser.error = 'Failed to browse directory';
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
                    if (this.localDirsDataKey) window[this.localDirsDataKey] = data.directories;
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
            if (!confirm('Remove this directory from context?')) return;

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
                    if (this.localDirsDataKey) window[this.localDirsDataKey] = data.directories;
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
                    if (this.localDirsDataKey) window[this.localDirsDataKey] = data.directories;
                    this.updateBadge();
                }
            } catch (err) {
                this.showStatus('Error toggling file', 'error');
            }
        },

        async viewLocalFile(dirPath, filename) {
            this.editModal.filename = filename;
            this.editModal.readOnly = true;
            this.editModal.status = '';

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
                    if (this.localDirsDataKey) window[this.localDirsDataKey] = data.directories;
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
        statusMessage: '',
        statusType: '',
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
                this.statusMessage = 'Failed to load models. Please try again.';
                this.statusType = 'error';
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
            this.statusMessage = '';

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
                    this.statusMessage = 'Provider and model updated successfully!';
                    this.statusType = 'success';
                    this.currentProvider = data.provider;
                    this.currentModel = data.model;
                    this.hasExistingKey = true;
                    this.apiKeyModified = false;
                    setTimeout(() => { this.statusMessage = ''; }, 3000);
                } else {
                    this.statusMessage = data.error || 'Failed to save settings';
                    this.statusType = 'error';
                }
            } catch (e) {
                this.statusMessage = 'Failed to save settings. Please try again.';
                this.statusType = 'error';
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
        personaItems: [],
        personaModels: {},
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
                this.defaultModel = dataEl.dataset.defaultModel || '';
            }

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
