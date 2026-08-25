---@meta

-- Hand-written prototype of what `installua stubs` will generate.
-- Phase 0 only: this exists to verify the LuaLS claims, not to be shipped.

--------------------------------------------------------------------------------
-- Enums, as `---@alias`. The claim under test: an alias of string literals
-- gives completion *inside* a string literal.
--------------------------------------------------------------------------------

---@alias installua.PageName
---| '"Welcome"'
---| '"License"'
---| '"Components"'
---| '"Directory"'
---| '"InstFiles"'
---| '"Finish"'

---@alias installua.UnPageName
---| '"Confirm"'
---| '"InstFiles"'
---| '"Finish"'

---@alias installua.ExecutionLevel
---| '"none"'
---| '"user"'
---| '"highest"'
---| '"admin"'

---@alias installua.CrcCheck
---| '"on"'
---| '"off"'
---| '"force"'

---@alias installua.MessageBoxButtons
---| '"OK"'
---| '"OKCANCEL"'
---| '"YESNO"'
---| '"YESNOCANCEL"'
---| '"RETRYCANCEL"'
---| '"ABORTRETRYIGNORE"'

--------------------------------------------------------------------------------
-- Blocks. `(exact)` is aspirational: LuaLS 3.19.1 does not check the
-- extra-field direction on a table constructor (finding L1). Kept so the check
-- starts working the day it lands.
--------------------------------------------------------------------------------

---@class (exact) installua.Manifest
---@field dpiAwareness? string
---@field gdiScaling? boolean
---@field supportedOS? string

---@class (exact) installua.Attributes
---@field name string
---@field outFile string
---@field unicode? boolean
---@field compressor? string
---@field requestExecutionLevel? installua.ExecutionLevel
---@field crcCheck? installua.CrcCheck
---@field xpStyle? boolean
---@field manifest? installua.Manifest

---@class (exact) installua.Installer
---@field installDir? string
---@field caption? string
---@field icon? string
---@field pages? installua.PageName[]

---@class (exact) installua.Uninstaller
---@field caption? string
---@field icon? string
---@field text? string
---@field pages? installua.UnPageName[]

---@param options installua.Attributes
function attributes(options) end

---@param options installua.Installer
function installer(options) end

---@param options installua.Uninstaller
function uninstaller(options) end

--------------------------------------------------------------------------------
-- Declarations
--------------------------------------------------------------------------------

---@param name string
---@param body fun()
function section(name, body) end

---@param body fun()
function onInit(body) end

--------------------------------------------------------------------------------
-- Instructions
--------------------------------------------------------------------------------

---@param text string
function detailPrint(text) end

---@param path string
function setOutPath(path) end

---@param pattern string
function file(pattern) end

---@param message? string
function abort(message) end

---@class (exact) installua.MessageBox
---@field text string
---@field buttons installua.MessageBoxButtons
---@field icon? string

---@param options installua.MessageBox
---@return string
function messageBox(options) end

--------------------------------------------------------------------------------
-- Predefined NSIS constants, as ordinary globals
--------------------------------------------------------------------------------

---@type string
INSTDIR = nil
---@type string
DESKTOP = nil
---@type string
PROGRAMFILES64 = nil
