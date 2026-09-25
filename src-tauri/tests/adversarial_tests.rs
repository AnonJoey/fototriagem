#[cfg(test)]
mod tests {
    use fototriagem_lib::actions::{GerenciadorAcoes, Movimento};
    use fototriagem_lib::library::{carregar_biblioteca, Copia};
    use std::fs;
    use std::path::PathBuf;

    struct TestEnv {
        root: PathBuf,
        origem: PathBuf,
        destino: PathBuf,
    }

    impl TestEnv {
        fn new(prefix: &str) -> Self {
            let root = std::env::temp_dir().join(format!("fototriagem-adv-{}-{}", prefix, std::process::id()));
            let _ = fs::remove_dir_all(&root);
            let origem = root.join("origem");
            let destino = root.join("destino");
            fs::create_dir_all(&origem).unwrap();
            fs::create_dir_all(&destino).unwrap();
            Self { root, origem, destino }
        }
    }

    impl Drop for TestEnv {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    /// BUG 1: nome de arquivo acentuado cuja truncagem de 46 bytes cai no meio
    /// de um caractere. E o caso comum aqui: foto com nome em portugues.
    /// O teste roda a varredura de verdade; se `variantes_do_nome` voltar a
    /// cortar por byte, isto entra em panico em vez de ficar vermelho educado.
    #[test]
    fn nome_acentuado_nao_derruba_a_varredura() {
        let env = TestEnv::new("utf8-nome");

        // 45 bytes ASCII, depois um 'c' cedilha ocupando os bytes 45 e 46:
        // qualquer corte em 46 cai exatamente no meio dele.
        let nome = format!("{}çoes_da_viagem_de_verao.jpg", "a".repeat(45));
        assert!(nome.len() > 51, "o nome precisa passar do limite que dispara a truncagem");
        assert!(!nome.is_char_boundary(46), "o byte 46 tem que cair no meio de um caractere");

        let midia = env.origem.join(&nome);
        fs::write(&midia, b"bytes da foto").unwrap();
        fs::write(
            env.origem.join(format!("{}.json", nome)),
            format!(r#"{{"title":"{}","photoTakenTime":{{"timestamp":"1718454600"}}}}"#, nome),
        )
        .unwrap();

        let scan = carregar_biblioteca(&env.origem, &env.destino, false);

        let item = scan
            .itens
            .iter()
            .find(|i| i.nome == nome)
            .expect("a foto de nome acentuado deveria ter sido varrida");
        assert_eq!(item.timestamp, 1718454600, "e datada pelo sidecar dela");
    }

    /// BUG 2: DATA LOSS IN UNDO (desfazer) - Silent overwrite of preexisting file at copia.origem
    #[test]
    fn test_undo_silently_destroys_file_at_copia_origem() {
        let env = TestEnv::new("undo-overwrite");
        let album = env.origem.join("Album");
        fs::create_dir_all(&album).unwrap();

        let midia = env.origem.join("foto.jpg");
        let copia = album.join("foto_copia.jpg");
        fs::write(&midia, b"conteudo original da foto").unwrap();
        fs::write(&copia, b"conteudo da copia").unwrap();

        let target_dir = env.destino.join("Viagem");

        let mut acoes = GerenciadorAcoes::new(env.destino.clone());
        let copias = vec![Copia {
            caminho: copia.to_string_lossy().to_string(),
            sidecar: None,
        }];

        // Move a midia e a copia vai para _copias/Viagem/foto_copia.jpg
        acoes.mover(
            Movimento {
                caminho: midia.to_str().unwrap(),
                timestamp: Some(1700000000),
                sidecar: None,
                copias: &copias,
            },
            &target_dir,
        ).unwrap();

        assert!(!copia.exists(), "Copia deveria ter sido movida para quarentena");

        // Enquanto isso, o usuário ou outro programa criou um NOVO arquivo no mesmo caminho onde a cópia ficava!
        // Ex: O usuário baixou outro arquivo com mesmo nome "foto_copia.jpg"
        let conteudo_novo = b"ARQUIVO NOVO E IMPORTANTE DO USUARIO CRIADO NA ORIGEM";
        fs::write(&copia, conteudo_novo).unwrap();

        // Agora o usuário pressiona Desfazer (Ctrl+Z)!
        let undo_res = acoes.desfazer();
        assert!(undo_res.is_ok(), "Desfazer executou");

        // O que aconteceu com o arquivo novo importante do usuário?
        let conteudo_atual = fs::read(&copia).unwrap();
        
        // CORREÇÃO: O arquivo novo é PRESERVADO com sem_colisao!
        assert_eq!(
            conteudo_atual,
            conteudo_novo,
            "CORREÇÃO CONFIRMADA: O arquivo preexistente em copia.origem foi PRESERVADO pelo desfazer!"
        );
    }

    /// BUG 3: DATA LOSS IN UNDO (desfazer) - Silent overwrite of preexisting sidecar at sidecar_origem
    #[test]
    fn test_undo_silently_destroys_file_at_sidecar_origem() {
        let env = TestEnv::new("undo-sc-overwrite");
        let midia = env.origem.join("foto2.jpg");
        let sidecar = env.origem.join("foto2.jpg.json");
        fs::write(&midia, b"foto2 bytes").unwrap();
        fs::write(&sidecar, b"sidecar original").unwrap();

        let target_dir = env.destino.join("Viagem");
        let mut acoes = GerenciadorAcoes::new(env.destino.clone());

        acoes.mover(
            Movimento {
                caminho: midia.to_str().unwrap(),
                timestamp: Some(1700000000),
                sidecar: Some("foto2.jpg.json"),
                copias: &[],
            },
            &target_dir,
        ).unwrap();

        // Usuário recria um sidecar com edições manuais no caminho de origem
        let conteudo_novo_sidecar = b"NOVO SIDECAR COM DADOS MANUAIS DO USUARIO";
        fs::write(&sidecar, conteudo_novo_sidecar).unwrap();

        // Executa desfazer
        acoes.desfazer().unwrap();

        // O sidecar novo foi preservado pelo sem_colisao!
        let conteudo_restaurado = fs::read(&sidecar).unwrap();
        assert_eq!(
            conteudo_restaurado,
            conteudo_novo_sidecar,
            "CORREÇÃO CONFIRMADA: Sidecar preexistente foi preservado pelo desfazer com sem_colisao!"
        );
    }

    /// BUG 4: PERMANENT LOSS OF UNDO STACK - desfazer desempilha antes de mover
    #[test]
    fn test_undo_stack_permanently_lost_on_error() {
        let env = TestEnv::new("undo-stack-loss");
        let midia = env.origem.join("foto3.jpg");
        fs::write(&midia, b"foto3 bytes").unwrap();

        let target_dir = env.destino.join("Viagem");
        let mut acoes = GerenciadorAcoes::new(env.destino.clone());

        let movido = acoes.mover(
            Movimento {
                caminho: midia.to_str().unwrap(),
                timestamp: Some(1700000000),
                sidecar: None,
                copias: &[],
            },
            &target_dir,
        ).unwrap();

        assert_eq!(acoes.pilha.len(), 1);

        // Simula exclusão externa ou arquivo movido externamente:
        // O arquivo movido não está mais lá (ex: usuário abriu ou moveu para outra pasta temporariamente)
        let movido_path = PathBuf::from(&movido);
        fs::remove_file(&movido_path).unwrap();

        // Chama desfazer: falha com "Arquivo movido não existe mais no destino"
        let err = acoes.desfazer();
        assert!(err.is_err(), "Desfazer falhou como esperado");

        // CORREÇÃO: A pilha NÃO é descartada em caso de erro!
        assert_eq!(
            acoes.pilha.len(),
            1,
            "CORREÇÃO CONFIRMADA: A pilha preservou o item após falha no desfazer!"
        );

        // Se o arquivo estiver disponível novamente e o usuário tentar Ctrl+Z:
        fs::write(&movido_path, b"foto3 bytes restaurados pelo usuario").unwrap();
        let retry = acoes.desfazer();
        assert!(
            matches!(retry, Ok(Some(_))),
            "CORREÇÃO CONFIRMADA: Usuário consegue realizar Ctrl+Z com sucesso após resolver o problema!"
        );
    }

    /// BUG 5: FALSE DUPLICATE & MASS QUARANTINE OF ZERO-BYTE FILES
    #[test]
    fn test_zero_byte_files_false_duplicate_quarantine() {
        let env = TestEnv::new("zero-byte");
        let pasta_a = env.origem.join("Viagem A");
        let pasta_b = env.origem.join("Documentos B");
        fs::create_dir_all(&pasta_a).unwrap();
        fs::create_dir_all(&pasta_b).unwrap();

        // Duas fotos de 0 bytes em pastas completamente diferentes (ex: arquivos corrompidos ou placeholders)
        let f1 = pasta_a.join("foto_a.jpg");
        let f2 = pasta_b.join("doc_b.png");
        fs::write(&f1, b"").unwrap();
        fs::write(&f2, b"").unwrap();

        let scan = carregar_biblioteca(&env.origem, &env.destino, false);

        // O scanner tratou os dois arquivos completamente diferentes como cópias idênticas!
        // CORREÇÃO: Arquivos de 0 bytes NÃO são tratados como duplicatas!
        assert_eq!(scan.total_arquivos, 2);
        assert_eq!(scan.duplicatas_removidas, 0, "CORREÇÃO CONFIRMADA: 0 cópias removidas para arquivos de 0 bytes!");
        assert_eq!(scan.itens.len(), 2, "Ambos os itens foram preservados individualmente!");
    }

    /// BUG 6: nome de pasta de destino digitado pelo usuario escapando do
    /// destino. Chama a validacao que os comandos `move_item` e
    /// `create_target_folder` usam, e nao `Path::join` em literais: assim,
    /// quem afrouxar a regra derruba este teste.
    #[test]
    fn nome_de_pasta_que_escapa_do_destino_e_recusado() {
        use fototriagem_lib::is_valid_folder_name;

        for hostil in [
            "..",
            "../..",
            "../../etc",
            "/etc",
            "/",
            "sub/pasta",
            "sub\\pasta",
            "   ",
            "",
            "pasta<script>",
        ] {
            assert!(
                !is_valid_folder_name(hostil),
                "{:?} deveria ser recusado como pasta de destino",
                hostil
            );
        }

        for legitimo in ["Viagem", "Junho 2024", "Fotos da Ana", "ferias-2024", "Aniversario do Joao"] {
            assert!(
                is_valid_folder_name(legitimo),
                "{:?} e um nome de pasta legitimo e deveria passar",
                legitimo
            );
        }
    }

    /// BUG 7: SILENT NON-DELETION of source file when remove_file fails
    #[test]
    fn test_mover_arquivo_silently_ignores_failed_source_deletion() {
        use std::os::unix::fs::PermissionsExt;

        let env = TestEnv::new("remove-fail");
        let src_dir = env.origem.join("ro_dir");
        fs::create_dir_all(&src_dir).unwrap();

        let src_file = src_dir.join("photo.jpg");
        fs::write(&src_file, b"photo content").unwrap();

        let target_dir = env.destino.join("viagem");
        fs::create_dir_all(&target_dir).unwrap();
        let target_file = target_dir.join("photo.jpg");

        // Tornamos o diretório de origem somente-leitura (impede fs::remove_file e fs::rename)
        let ro_perms = fs::Permissions::from_mode(0o555);
        fs::set_permissions(&src_dir, ro_perms).unwrap();

        // Agora fs::rename falhará (PermissionDenied).
        // mover_arquivo tentará fs::copy (que funciona, pois leitura do arquivo é permitida).
        // Em seguida tentará fs::remove_file(&src_file) (que FALHARÁ, pois o diretório pai é somente-leitura)!
        // Mas mover_arquivo tem: let _ = fs::remove_file(de); ignorando o erro!
        // Vamos chamar GerenciadorAcoes::mover:
        let mut acoes = GerenciadorAcoes::new(env.destino.clone());
        let res = acoes.mover(
            Movimento {
                caminho: src_file.to_str().unwrap(),
                timestamp: None,
                sidecar: None,
                copias: &[],
            },
            &target_dir,
        );

        // Restaura permissões para limpeza
        let _ = fs::set_permissions(&src_dir, fs::Permissions::from_mode(0o755));

        // O movimento declarou SUCESSO!
        // CORREÇÃO: O movimento falha honestamente e limpa a cópia órfã no destino!
        assert!(res.is_err(), "CORREÇÃO CONFIRMADA: mover_arquivo reportou erro quando exclusão falhou!");
        assert!(!target_file.exists(), "CORREÇÃO CONFIRMADA: Cópia parcial no destino foi revertida para evitar duplicação fantasma!");
    }

    /// BUG 8: casamento de sidecar em pasta grande.
    ///
    /// A versao anterior deste teste exigia que a busca fosse LENTA
    /// (`elapsed > 10ms`), o que inverte o alvo: quem otimizasse a busca
    /// deixaria a suite vermelha. Aqui o que se cobra e o que importa, que a
    /// varredura ache todos os pares numa pasta grande.
    ///
    /// O teto de tempo e grosseiro de proposito, e nao prova nada sobre a
    /// complexidade da busca: medido nesta escala, trocar a lista de nomes
    /// por um conjunto nao muda o tempo, porque criar os 1200 pares no disco
    /// domina o relogio. Ele so pega uma regressao de ordem de grandeza.
    #[test]
    fn pasta_grande_casa_todos_os_sidecars_em_tempo_razoavel() {
        let env = TestEnv::new("pasta-grande");
        const N: usize = 1200;

        for i in 0..N {
            let nome = format!("IMG_{:05}.jpg", i);
            fs::write(env.origem.join(&nome), format!("bytes de {}", i)).unwrap();
            fs::write(
                env.origem.join(format!("{}.json", nome)),
                format!(
                    r#"{{"title":"{}","photoTakenTime":{{"timestamp":"{}"}}}}"#,
                    nome,
                    1718454600 + i as i64
                ),
            )
            .unwrap();
        }

        let inicio = std::time::Instant::now();
        let scan = carregar_biblioteca(&env.origem, &env.destino, false);
        let gasto = inicio.elapsed();

        assert_eq!(scan.itens.len(), N, "todas as fotos deveriam ter sido varridas");
        let sem_sidecar: Vec<_> = scan.itens.iter().filter(|i| i.sidecar.is_none()).collect();
        assert!(
            sem_sidecar.is_empty(),
            "{} fotos ficaram sem sidecar numa pasta onde todas tem",
            sem_sidecar.len()
        );
        assert!(
            gasto.as_secs() < 30,
            "varrer {} pares levou {:?}: busca de sidecar regrediu para tempo quadratico",
            N,
            gasto
        );
    }

    /// BUG 9: varrer de novo apagava a pilha de desfazer.
    ///
    /// A versao anterior construia um GerenciadorAcoes novo e verificava que
    /// a pilha dele estava vazia, o que so prova que um construtor inicializa
    /// um Vec vazio. Aqui se exercita `deve_preservar_pilha`, que e a mesma
    /// regra que `scan_library` consulta, mais o efeito dela sobre uma pilha
    /// de verdade.
    #[test]
    fn varrer_de_novo_no_mesmo_destino_preserva_o_desfazer() {
        use fototriagem_lib::deve_preservar_pilha;

        let env = TestEnv::new("revarredura");
        let midia = env.origem.join("foto_rescan.jpg");
        fs::write(&midia, b"bytes").unwrap();

        let mut acoes = GerenciadorAcoes::new(env.destino.clone());
        acoes
            .mover(
                Movimento {
                    caminho: midia.to_str().unwrap(),
                    timestamp: None,
                    sidecar: None,
                    copias: &[],
                },
                &env.destino.join("Viagem"),
            )
            .unwrap();
        assert_eq!(acoes.pilha.len(), 1, "ha uma decisao para desfazer");

        // Varrer de novo o MESMO destino: a pilha continua valendo.
        assert!(
            deve_preservar_pilha(Some(&acoes.destino), &env.destino),
            "revarrer o mesmo destino nao deveria descartar o desfazer"
        );

        // E ela realmente ainda funciona depois disso.
        let desfeito = acoes.desfazer().unwrap().expect("o desfazer ainda alcanca a decisao");
        assert_eq!(desfeito.caminho_restaurado, midia.to_string_lossy());
        assert!(midia.is_file(), "a foto voltou para a origem");

        // Trocar de destino e outra historia: ai a pilha antiga aponta para
        // outra arvore e descarta-la e o certo.
        let outro = env.root.join("outro_destino");
        assert!(
            !deve_preservar_pilha(Some(&acoes.destino), &outro),
            "mudar o destino deveria descartar a pilha antiga"
        );
        assert!(!deve_preservar_pilha(None, &env.destino), "sem destino anterior nao ha o que preservar");
    }

    /// BUG 10: corrida entre escolher o nome livre e mover.
    ///
    /// A versao anterior chamava `fs::rename` direto e provava que o Linux
    /// sobrescreve, que e POSIX e nao diz nada sobre este app. Aqui varias
    /// threads disputam o mesmo nome pelo `reservar_nome` de verdade: se ele
    /// voltar a ser um "olha e depois cria", duas delas recebem o mesmo
    /// caminho e o teste acusa.
    #[test]
    fn nomes_reservados_em_paralelo_nunca_se_repetem() {
        use std::collections::HashSet;
        use std::sync::Arc;

        let env = TestEnv::new("corrida-nome");
        let pasta = env.destino.join("Viagem");
        fs::create_dir_all(&pasta).unwrap();

        let acoes = Arc::new(GerenciadorAcoes::new(env.destino.clone()));
        const THREADS: usize = 16;

        let mut maos = Vec::new();
        for _ in 0..THREADS {
            let acoes = Arc::clone(&acoes);
            let pasta = pasta.clone();
            maos.push(std::thread::spawn(move || {
                acoes.reservar_nome(&pasta, "foto.jpg", None).expect("reserva deveria funcionar")
            }));
        }

        let obtidos: Vec<PathBuf> = maos.into_iter().map(|m| m.join().unwrap()).collect();
        let unicos: HashSet<&PathBuf> = obtidos.iter().collect();

        assert_eq!(
            unicos.len(),
            THREADS,
            "duas threads receberam o mesmo caminho: a reserva do nome nao e atomica, e um movimento passaria por cima do outro"
        );
        for caminho in &obtidos {
            assert!(caminho.is_file(), "cada nome reservado deveria existir no disco: {:?}", caminho);
        }
    }

    /// BUG 11: prevencao de sobreposicao de pastas entre origem e destino.
    ///
    /// Recusa quando origem e destino sao a mesma pasta, ou quando uma e
    /// subpasta da outra, evitando recursao e corrupcao de arquivos.
    #[test]
    fn origem_e_destino_iguais_sao_recusados() {
        use fototriagem_lib::library::validar_caminhos_origem_destino;

        let env = TestEnv::new("dir-iguais");
        let res = validar_caminhos_origem_destino(&env.origem, &env.origem);
        assert!(res.is_err(), "origem e destino iguais devem ser recusados");

        // A regra vale para MOVIMENTO, nao para varredura, e por isso este
        // teste nao afirma mais nada sobre `carregar_biblioteca`.
        //
        // Ele afirmava, e essa assercao contradizia um teste mais antigo que
        // passava: `o_que_a_varredura_achou_e_o_que_chega_no_destino` revarre a
        // pasta de destino passando ela como origem e destino, so para
        // conferir o que chegou la. Varrer e leitura e nao corrompe nada; a
        // recursao que a regra evita nasce de mover arquivo para dentro da
        // propria origem, e isso e barrado em `lib.rs` antes da varredura.
        //
        // Os dois testes nao podiam estar certos ao mesmo tempo. Este cedeu
        // porque era o novo e porque afirmava sobre a fronteira errada.
    }

    #[test]
    fn destino_subpasta_da_origem_e_recusado() {
        use fototriagem_lib::library::validar_caminhos_origem_destino;

        let env = TestEnv::new("dir-sub-dest");
        let sub = env.origem.join("organizadas");
        let res = validar_caminhos_origem_destino(&env.origem, &sub);
        assert!(res.is_err(), "destino dentro da origem deve ser recusado");
    }

    #[test]
    fn origem_subpasta_do_destino_e_recusado() {
        use fototriagem_lib::library::validar_caminhos_origem_destino;

        let env = TestEnv::new("dir-sub-orig");
        let sub = env.destino.join("fotos_brutas");
        fs::create_dir_all(&sub).unwrap();
        let res = validar_caminhos_origem_destino(&sub, &env.destino);
        assert!(res.is_err(), "origem dentro do destino deve ser recusada");
    }

    #[test]
    fn origem_e_destino_distintos_sao_aceitos() {
        use fototriagem_lib::library::validar_caminhos_origem_destino;

        let env = TestEnv::new("dir-distintos");
        let res = validar_caminhos_origem_destino(&env.origem, &env.destino);
        assert!(res.is_ok(), "pastas irmas distintas devem ser aceitas com sucesso");
    }

    #[test]
    fn status_dos_caminhos_sugeridos_e_valido() {
        use fototriagem_lib::caminhos_sugeridos;

        let (orig_sug, dest_sug, lista) = caminhos_sugeridos();
        assert!(!orig_sug.is_empty(), "origem sugerida deve estar preenchida");
        assert!(!dest_sug.is_empty(), "destino sugerido deve estar preenchido");
        assert!(!lista.is_empty(), "lista de sugeridos deve conter caminhos");
        assert_ne!(orig_sug, dest_sug, "origem e destino sugeridos nao podem ser iguais");
    }

}
