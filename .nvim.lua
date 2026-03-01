vim.filetype.add({
  extension = {
    code = 'baselang',
  },
})

vim.api.nvim_create_autocmd('FileType', {
  pattern = 'baselang',
  callback = function()
    vim.lsp.start({
      name = 'baselang',
      cmd = { vim.fn.expand('~') .. '/Projects/baselang/target/debug/baselang-lsp' },
      root_dir = vim.fn.getcwd(),
    })
  end,
})

vim.api.nvim_create_autocmd('LspAttach', {
  callback = function(args)
    local client = vim.lsp.get_client_by_id(args.data.client_id)
    if client and client.name == 'baselang' then
      -- LazyVim's snacks_picker sets gd two event-loop ticks after LspAttach.
      -- We schedule three times to run after it.
      vim.schedule(function()
        vim.schedule(function()
          vim.schedule(function()
            vim.keymap.set('n', 'gd', function()
              Snacks.picker.lsp_definitions({ include_current = true })
            end, { buffer = args.buf })
          end)
        end)
      end)
    end
  end,
})
