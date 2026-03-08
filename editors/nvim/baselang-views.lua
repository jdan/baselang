-- baselang-views.lua
--
-- Projectional editing for baselang: display $-identifiers using
-- human-readable names from a .view sidecar file, plus per-line
-- observability loaded from a .observability.json sidecar.

local M = {}
local ns = vim.api.nvim_create_namespace("baselang_views")

local heatmap_groups = {
  "BaselangObsCold",
  "BaselangObsCool",
  "BaselangObsWarm",
  "BaselangObsHot",
  "BaselangObsBlaze",
}

M.current_mode = "main"
M.enabled = true

local cursor_lines = {}
local metrics_by_buf = {}
local metric_width = 15

local function with_suffix(path, suffix)
  return path .. suffix
end

local function read_json_file(path)
  local f = io.open(path, "r")
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

local function current_source(buf)
  local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
  local source = table.concat(lines, "\n")
  if vim.bo[buf].endofline then
    source = source .. "\n"
  end
  return source
end

local function format_duration(avg_nanos)
  if avg_nanos >= 1000000000 then
    return string.format("%.2fs", avg_nanos / 1000000000)
  end
  if avg_nanos >= 1000000 then
    return string.format("%.1fms", avg_nanos / 1000000)
  end
  if avg_nanos >= 1000 then
    return string.format("%.1fus", avg_nanos / 1000)
  end
  return string.format("%dns", avg_nanos)
end

local function format_count(count)
  if count >= 1000000 then
    return string.format("%.1fm", count / 1000000)
  end
  if count >= 1000 then
    return string.format("%.1fk", count / 1000)
  end
  return tostring(count)
end

local function load_metrics(buf)
  local filename = vim.api.nvim_buf_get_name(buf)
  if filename == "" then
    metrics_by_buf[buf] = nil
    return
  end

  local report = read_json_file(with_suffix(filename, ".observability.json"))
  if not report then
    metrics_by_buf[buf] = nil
    return
  end

  if report.file_hash ~= vim.fn.sha256(current_source(buf)) then
    metrics_by_buf[buf] = nil
    return
  end

  local lines = {}
  local max_count = 0
  local max_avg_nanos = 0
  for _, entry in ipairs(report.lines or {}) do
    lines[entry.line] = entry
    max_count = math.max(max_count, entry.count or 0)
    max_avg_nanos = math.max(max_avg_nanos, entry.avg_nanos or 0)
  end

  metrics_by_buf[buf] = {
    lines = lines,
    max_count = max_count,
    max_avg_nanos = max_avg_nanos,
  }
end

function M.metric_text(buf, lnum)
  local metrics = metrics_by_buf[buf]
  if not metrics then
    return string.rep(" ", metric_width)
  end

  local entry = metrics.lines[lnum]
  if not entry then
    return string.rep(" ", metric_width)
  end

  local text = string.format("%sx %s", format_count(entry.count), format_duration(entry.avg_nanos))
  if #text > metric_width then
    return text:sub(1, metric_width)
  end
  return string.rep(" ", metric_width - #text) .. text
end

function M.metric_highlight(buf, lnum)
  local metrics = metrics_by_buf[buf]
  if not metrics then
    return "LineNr"
  end

  local entry = metrics.lines[lnum]
  if not entry then
    return "LineNr"
  end

  local count_ratio = 0
  local time_ratio = 0

  if metrics.max_count > 0 then
    count_ratio = entry.count / metrics.max_count
  end
  if metrics.max_avg_nanos > 0 then
    time_ratio = entry.avg_nanos / metrics.max_avg_nanos
  end

  local score = math.max(count_ratio, time_ratio)
  if score >= 0.85 then
    return heatmap_groups[5]
  end
  if score >= 0.55 then
    return heatmap_groups[4]
  end
  if score >= 0.30 then
    return heatmap_groups[3]
  end
  if score > 0 then
    return heatmap_groups[2]
  end
  return heatmap_groups[1]
end

function M.statuscolumn()
  if vim.v.virtnum ~= 0 then
    return ""
  end

  local buf = vim.api.nvim_get_current_buf()
  local hl = M.metric_highlight(buf, vim.v.lnum)
  return "%#" .. hl .. "#" .. M.metric_text(buf, vim.v.lnum) .. "%#LineNr# " .. string.format("%4d ", vim.v.lnum)
end

function M.load_views(buf)
  local filename = vim.api.nvim_buf_get_name(buf)
  if filename == "" then
    return nil
  end

  return read_json_file(with_suffix(filename, ".view"))
end

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

local function clear_line(buf, lnum)
  local marks = vim.api.nvim_buf_get_extmarks(buf, ns, { lnum, 0 }, { lnum, -1 }, {})
  for _, m in ipairs(marks) do
    vim.api.nvim_buf_del_extmark(buf, ns, m[1])
  end
end

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

  if prev then
    clear_line(buf, prev)
    local lines = vim.api.nvim_buf_get_lines(buf, prev, prev + 1, false)
    if #lines > 0 then
      apply_line(buf, prev, lines[1], views, M.current_mode)
    end
  end

  clear_line(buf, cursor_line)
  cursor_lines[buf] = cursor_line
end

local function refresh(buf)
  load_metrics(buf)
  if vim.api.nvim_buf_is_valid(buf) then
    vim.opt_local.number = true
    vim.opt_local.numberwidth = 4
    vim.opt_local.statuscolumn = "%!v:lua.require'baselang-views'.statuscolumn()"
    if M.enabled and M.load_views(buf) then
      vim.opt_local.conceallevel = 3
    end
    M.apply_views(buf)
    vim.cmd("redrawstatus")
  end
end

local function set_heatmap_highlights()
  vim.api.nvim_set_hl(0, "BaselangObsCold", { fg = "#5f6b76" })
  vim.api.nvim_set_hl(0, "BaselangObsCool", { fg = "#6ea8a1", bold = true })
  vim.api.nvim_set_hl(0, "BaselangObsWarm", { fg = "#d1a65a", bold = true })
  vim.api.nvim_set_hl(0, "BaselangObsHot", { fg = "#dd7a4b", bold = true })
  vim.api.nvim_set_hl(0, "BaselangObsBlaze", { fg = "#d14d41", bold = true })
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
  set_heatmap_highlights()

  vim.api.nvim_create_autocmd("ColorScheme", {
    callback = set_heatmap_highlights,
  })

  vim.api.nvim_create_autocmd({ "BufEnter", "BufWritePost", "TextChanged", "TextChangedI", "CursorHold", "FocusGained" }, {
    pattern = "*.code",
    callback = function(ev)
      refresh(ev.buf)
    end,
  })

  vim.api.nvim_create_autocmd({ "CursorMoved", "CursorMovedI" }, {
    pattern = "*.code",
    callback = function(ev)
      on_cursor_moved(ev.buf)
    end,
  })

  vim.api.nvim_create_autocmd("BufLeave", {
    pattern = "*.code",
    callback = function(ev)
      metrics_by_buf[ev.buf] = nil
      cursor_lines[ev.buf] = nil
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
