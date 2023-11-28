use discorhdd::{
    discord_drive,
    test_bridge::{test_config, TestBridge},
};

#[test]
fn main_tests() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let mut chunk_man = discord_drive::ChunkManager::new(1_000);
        chunk_man.save_to_file = false;
        let mut discord_drive = discord_drive::DiscordDrive::<TestBridge>::new(test_config())
            .await
            .unwrap();

        assert_eq!(
            chunk_man.read(0, 10, &mut discord_drive).await.unwrap(),
            vec![0; 10]
        );
        assert_eq!(
            chunk_man.read(0, 1_005, &mut discord_drive).await.unwrap(),
            vec![0; 1_005]
        );
        chunk_man
            .write(0, &[1; 7], &mut discord_drive)
            .await
            .unwrap();
        assert_eq!(
            chunk_man.read(0, 10, &mut discord_drive).await.unwrap(),
            vec![1, 1, 1, 1, 1, 1, 1, 0, 0, 0]
        );
        chunk_man
            .write(16, &[1; 1024], &mut discord_drive)
            .await
            .unwrap();
        assert_eq!(
            chunk_man.read(16, 1024, &mut discord_drive).await.unwrap(),
            vec![1; 1024]
        );
    });
}
