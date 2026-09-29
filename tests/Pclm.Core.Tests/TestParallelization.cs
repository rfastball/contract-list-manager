using Xunit;

// 시험 클래스마다 정리에서 SqliteConnection.ClearAllPools() 를 부른다. 그것은 프로세스 전체의
// 풀을 비우므로, 다른 클래스가 병렬로 막 연 연결까지 폐기해 ObjectDisposedException 이 가끔
// 났다(실측 — DurableKeyTests 가 네 번에 한 번꼴). 풀을 클래스마다 가를 길이 없어 클래스 사이의
// 병렬을 끈다. 전체 시험이 한 줄로 도는 대가는 실행 시간뿐이다.
[assembly: CollectionBehavior(DisableTestParallelization = true)]
