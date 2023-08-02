.FORCE: 

readme: .FORCE
	cat README_start.md > README.md
	cat examples/callback.rs >> README.md
	cat README_end.md >> README.md
	

