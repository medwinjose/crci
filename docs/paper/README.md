# CRCI Research Paper

This directory contains the final research paper draft for the Crisis Response Communication Infrastructure (CRCI) project. The paper (`crci_paper.md`) is written in Markdown and is structured to match the standard format for systems and networking workshop submissions (e.g., HotNets, ACM SIGCOMM, IEEE WoWMoM). To compile the paper into a LaTeX format suitable for an arXiv or IEEE/ACM submission, you can use Pandoc by running `pandoc -s crci_paper.md -o crci_paper.tex`. From there, the generated `.tex` file can be imported into Overleaf or compiled locally with `pdflatex` using the target venue's two-column style template.

## arXiv Submission Checklist

Before submitting to arXiv, ensure the following steps are completed:
- [ ] **Benchmark Data:** Replace all `[TODO: replace with cargo bench output]` placeholders in `crci_paper.md` with final measured latency and throughput numbers.
- [ ] **LaTeX Conversion:** Run `pandoc -s crci_paper.md -o crci_paper.tex` and verify the output.
- [ ] **Template Integration:** Wrap the generated LaTeX in the appropriate arXiv or IEEE conference template (e.g., `\documentclass[10pt, conference]{IEEEtran}`).
- [ ] **Figures and Diagrams:** Export any relevant mermaid diagrams (like the architecture or state machine diagrams from the repository) as high-resolution PDFs or EPS files and insert them into the LaTeX document.
- [ ] **Citation Verification:** Ensure all bibliography entries are properly formatted using BibTeX and linked in the text.
- [ ] **Author Metadata:** Add full author affiliations, email addresses, and the primary contact for correspondence.
- [ ] **Abstract Submission:** Copy the plain-text abstract directly into the arXiv submission portal metadata fields.
