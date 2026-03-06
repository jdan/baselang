-- baselang-views.lua
--
-- Projectional editing for baselang: display $-identifiers using
-- human-readable names from a .view sidecar file.
--
-- The cursor line always shows raw $IDs for editing.
-- All other lines show projected names.
--
-- Setup:
--   require("baselang-views").setup()
--
-- Commands:
--   :BaselangView           -- list available view modes
--   :BaselangView main      -- switch to "main" view
--   :BaselangView maths     -- switch to "maths" view
--   :BaselangView off       -- disable views (show raw $IDs)

local M = {}
local ns = vim.api.nvim_create_namespace("baselang_views")

M.current_mode = "main"
M.enabled = true

-- Track cursor line per buffer to avoid redundant work
local cursor_lines = {}

function M.load_views(buf)
  local filename = vim.api.nvim_buf_get_name(buf)
  if filename == "" then
    return nil
  end

  local f = io.open(filename .. ".view", "r")
  if not f then
    return nil
  end

  local content = f:read("*a")
  f:close()

  local ok, data = pcall(vim.json.decode, content)
  if not ok then
    return nil
  end

  return data
end

-- Apply extmarks for a single line
local function apply_line(buf, lnum, line, views, mode)
  local start = 1
  while true do
    local s, e, id = line:find("(%$%w+)", start)
    if not s then
      break
    end

    local view_entry = views[id]
    if view_entry and view_entry[mode] then
      vim.api.nvim_buf_set_extmark(buf, ns, lnum, s - 1, {
        end_col = e,
        conceal = "",
        virt_text = { { view_entry[mode], "@variable" } },
        virt_text_pos = "inline",
      })
    end

    start = e + 1
  end
end

-- Clear extmarks on a single line
local function clear_line(buf, lnum)
  local marks = vim.api.nvim_buf_get_extmarks(buf, ns, { lnum, 0 }, { lnum, -1 }, {})
  for _, m in ipairs(marks) do
    vim.api.nvim_buf_del_extmark(buf, ns, m[1])
  end
end

-- Apply views to entire buffer, skipping the cursor line
function M.apply_views(buf, mode)
  mode = mode or M.current_mode
  vim.api.nvim_buf_clear_namespace(buf, ns, 0, -1)

  if not M.enabled then
    return
  end

  local views = M.load_views(buf)
  if not views then
    return
  end

  local cursor_line = vim.api.nvim_win_get_cursor(0)[1] - 1
  cursor_lines[buf] = cursor_line

  local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
  for lnum, line in ipairs(lines) do
    if lnum - 1 ~= cursor_line then
      apply_line(buf, lnum - 1, line, views, mode)
    end
  end
end

-- Handle cursor movement: reveal raw IDs on cursor line, project on previous line
local function on_cursor_moved(buf)
  if not M.enabled then
    return
  end

  local views = M.load_views(buf)
  if not views then
    return
  end

  local cursor_line = vim.api.nvim_win_get_cursor(0)[1] - 1
  local prev = cursor_lines[buf]

  if cursor_line == prev then
    return
  end

  -- Restore projections on the line we just left
  if prev then
    clear_line(buf, prev)
    local lines = vim.api.nvim_buf_get_lines(buf, prev, prev + 1, false)
    if #lines > 0 then
      apply_line(buf, prev, lines[1], views, M.current_mode)
    end
  end

  -- Clear projections on the line we moved to
  clear_line(buf, cursor_line)

  cursor_lines[buf] = cursor_line
end

function M.set_mode(mode)
  if mode == "off" then
    M.enabled = false
    local buf = vim.api.nvim_get_current_buf()
    vim.api.nvim_buf_clear_namespace(buf, ns, 0, -1)
    vim.opt_local.conceallevel = 0
    return
  end

  M.enabled = true
  M.current_mode = mode
  local buf = vim.api.nvim_get_current_buf()
  vim.opt_local.conceallevel = 3
  M.apply_views(buf, mode)
end

function M.list_modes()
  local buf = vim.api.nvim_get_current_buf()
  local views = M.load_views(buf)
  if not views then
    print("No .view file found")
    return
  end

  local modes = {}
  for _, entry in pairs(views) do
    for mode, _ in pairs(entry) do
      modes[mode] = true
    end
  end

  local mode_list = {}
  for mode, _ in pairs(modes) do
    table.insert(mode_list, mode)
  end
  table.sort(mode_list)

  local status = M.enabled and M.current_mode or "off"
  print("Available views: " .. table.concat(mode_list, ", ") .. " (current: " .. status .. ")")
end

local function complete_modes()
  local buf = vim.api.nvim_get_current_buf()
  local views = M.load_views(buf)
  local result = { "off" }
  if views then
    local modes = {}
    for _, entry in pairs(views) do
      for mode, _ in pairs(entry) do
        modes[mode] = true
      end
    end
    for mode, _ in pairs(modes) do
      table.insert(result, mode)
    end
  end
  table.sort(result)
  return result
end

function M.setup()
  vim.api.nvim_create_autocmd({ "BufEnter", "BufWritePost", "TextChanged", "TextChangedI" }, {
    pattern = "*.code",
    callback = function(ev)
      if M.enabled and M.load_views(ev.buf) then
        vim.opt_local.conceallevel = 3
      end
      M.apply_views(ev.buf)
    end,
  })

  vim.api.nvim_create_autocmd({ "CursorMoved", "CursorMovedI" }, {
    pattern = "*.code",
    callback = function(ev)
      on_cursor_moved(ev.buf)
    end,
  })

  vim.api.nvim_create_user_command("BaselangView", function(opts)
    if opts.args == "" then
      M.list_modes()
    else
      M.set_mode(opts.args)
    end
  end, {
    nargs = "?",
    complete = function()
      return complete_modes()
    end,
  })
end

return M
