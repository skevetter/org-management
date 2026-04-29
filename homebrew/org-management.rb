class OrgManagement < Formula
  desc "CLI and MCP server for tracking agent hierarchy and artifacts"
  homepage "https://github.com/skevetter/org-management"
  url "https://github.com/skevetter/org-management/archive/refs/tags/v0.1.0.tar.gz"
  sha256 ""
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "org-management", shell_output("#{bin}/org-management --help")
  end
end
