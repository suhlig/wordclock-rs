-- Render ```mermaid fenced blocks to PNG before pandoc writes the book.
--
-- The build script provides two environment variables:
--
--   MERMAID_CLI        command that runs mermaid-cli (default: mmdc)
--   MERMAID_CACHE_DIR  directory for rendered diagrams
--
-- PNG is used instead of SVG for two reasons: mermaid's HTML labels live in
-- <foreignObject>, which many EPUB readers do not render, and disabling HTML
-- labels makes mermaid drop the spaces between words. A 3x raster keeps the
-- text sharp at book sizes and works in every reader.
--
-- Renders are keyed by the SHA-1 of the diagram source, so the EPUB and PDF
-- passes share one render and unchanged diagrams are not rendered again.

local cache_dir = os.getenv("MERMAID_CACHE_DIR") or "dist/.mermaid-cache"
local cli = os.getenv("MERMAID_CLI") or "mmdc"

local function quote(value)
  return "'" .. value:gsub("'", "'\\''") .. "'"
end

local function exists(path)
  local file = io.open(path, "rb")
  if file then
    file:close()
    return true
  end
  return false
end

local function render(source)
  os.execute("mkdir -p " .. quote(cache_dir))

  local hash = pandoc.utils.sha1(source .. "\n" .. cli .. "\nscale=3")
  local png = cache_dir .. "/" .. hash .. ".png"
  if exists(png) then
    return png
  end

  local input = cache_dir .. "/" .. hash .. ".mmd"
  local file = assert(io.open(input, "wb"))
  file:write(source)
  file:close()

  local command = cli
    .. " -q -s 3 -b white -i "
    .. quote(input)
    .. " -o "
    .. quote(png)

  local ok, _, code = os.execute(command)
  if not ok or not exists(png) then
    error(
      "mermaid rendering failed (exit "
        .. tostring(code)
        .. "): "
        .. command
        .. "\nInstall @mermaid-js/mermaid-cli or set MERMAID_CLI."
    )
  end

  return png
end

function CodeBlock(block)
  if not block.classes:includes("mermaid") then
    return nil
  end

  local image = pandoc.Image(
    { pandoc.Str("Architecture diagram") },
    render(block.text),
    "",
    pandoc.Attr("", {}, { width = "100%" })
  )
  return pandoc.Para({ image })
end
