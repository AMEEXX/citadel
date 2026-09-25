use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TestCase {
    pub input: String,
    pub expected_output: String,
    pub explanation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuestionSummary {
    pub id: String,
    pub number: u32,
    pub title: String,
    pub difficulty: String,
    pub points: u32,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Question {
    pub id: String,
    pub number: u32,
    pub title: String,
    pub difficulty: String,
    pub points: u32,
    pub tags: Vec<String>,
    pub description: String,
    pub input_format: String,
    pub output_format: String,
    pub constraints: Vec<String>,
    pub sample_cases: Vec<TestCase>,
    pub starter_templates: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExamInfo {
    pub exam_id: String,
    pub title: String,
    pub institution: String,
    pub network_mode: String,
    pub duration_minutes: u32,
    pub total_questions: u32,
    pub total_points: u32,
    pub instructions: Vec<String>,
}

pub fn get_exam_info() -> ExamInfo {
    ExamInfo {
        exam_id: "citadel-campus-2026-drive".to_string(),
        title: "Campus Placement Technical Assessment 2026".to_string(),
        institution: "CITADEL Secure Offline Assessment Fleet".to_string(),
        network_mode: "Offline Campus Wi-Fi Only (Air-Gapped, Zero Internet)".to_string(),
        duration_minutes: 90,
        total_questions: 3,
        total_points: 100,
        instructions: vec![
            "This assessment runs entirely on the local college Wi-Fi network with no external internet connection.".to_string(),
            "All candidate traffic is restricted to this exam appliance IP address.".to_string(),
            "You may solve questions in Python, C++, or Java.".to_string(),
            "Test your solution against the sample test cases before submitting.".to_string(),
            "Code submissions are evaluated deterministically in the local sandbox.".to_string(),
        ],
    }
}

pub fn get_all_questions() -> Vec<Question> {
    vec![
        // Question 1
        Question {
            id: "q1-token-bucket".to_string(),
            number: 1,
            title: "Distributed Token Bucket Rate Limiter".to_string(),
            difficulty: "Medium".to_string(),
            points: 30,
            tags: vec!["Algorithms".to_string(), "Rate Limiting".to_string(), "Systems".to_string()],
            description: "Implement an offline Token Bucket rate limiting algorithm. The bucket has a maximum capacity `C` (tokens) and continuously refills at rate `R` tokens per second. At time `t = 0`, the bucket starts full with `C` tokens.\n\nYou are given a sequence of incoming request timestamps `[t_1, t_2, ..., t_n]` (integers, in seconds). Each request costs exactly `1` token. If the bucket has at least `1` token at time `t_i`, the request is ACCEPTED (`1`) and 1 token is deducted. Otherwise, the request is REJECTED (`0`).\n\nNote: Tokens accumulate continuously up to capacity `C`. For example, after `delta_t` seconds, `min(C, current_tokens + delta_t * R)` tokens are available.".to_string(),
            input_format: "Line 1: Two integers C (capacity) and R (refill rate per second).\nLine 2: An integer N (number of requests).\nLine 3: N space-separated integers representing request timestamps in non-decreasing order.".to_string(),
            output_format: "A space-separated sequence of 1s (Accepted) and 0s (Rejected).".to_string(),
            constraints: vec![
                "1 <= C <= 1000".to_string(),
                "1 <= R <= 100".to_string(),
                "1 <= N <= 100,000".to_string(),
                "0 <= t_1 <= t_2 <= ... <= t_n <= 10^9".to_string(),
            ],
            sample_cases: vec![
                TestCase {
                    input: "3 1\n5\n0 0 0 0 2".to_string(),
                    expected_output: "1 1 1 0 1".to_string(),
                    explanation: Some("At t=0, bucket has 3 tokens. First 3 requests accept (tokens: 3->2->1->0). 4th request at t=0 rejects (0 tokens). At t=2, 2 seconds elapsed: 2 tokens refilled, so 5th request accepts.".to_string()),
                },
                TestCase {
                    input: "2 2\n4\n1 1 1 2".to_string(),
                    expected_output: "1 1 0 1".to_string(),
                    explanation: Some("At t=1, bucket is capped at 2 tokens. Requests 1 and 2 accept; request 3 at t=1 rejects. At t=2, 2 tokens refilled, request 4 accepts.".to_string()),
                },
            ],
            starter_templates: {
                let mut m = HashMap::new();
                m.insert("python".to_string(), "# Solution for Distributed Token Bucket Rate Limiter\nimport sys\n\ndef solve():\n    lines = sys.stdin.read().split()\n    if not lines:\n        return\n    C = int(lines[0])\n    R = int(lines[1])\n    N = int(lines[2])\n    timestamps = [int(x) for x in lines[3:3+N]]\n    \n    # Your logic here\n    # Output space-separated 1s and 0s\n    results = []\n    print(\" \".join(map(str, results)))\n\nif __name__ == '__main__':\n    solve()\n".to_string());
                m.insert("cpp".to_string(), "// Solution for Distributed Token Bucket Rate Limiter\n#include <iostream>\n#include <vector>\n#include <algorithm>\n\nusing namespace std;\n\nint main() {\n    ios_base::sync_with_stdio(false);\n    cin.tie(NULL);\n    long long C, R;\n    int N;\n    if (!(cin >> C >> R >> N)) return 0;\n    vector<long long> t(N);\n    for (int i = 0; i < N; ++i) cin >> t[i];\n    \n    // Your logic here\n    \n    return 0;\n}\n".to_string());
                m.insert("java".to_string(), "// Solution for Distributed Token Bucket Rate Limiter\nimport java.util.Scanner;\n\npublic class Solution {\n    public static void main(String[] args) {\n        Scanner sc = new Scanner(System.in);\n        if (!sc.hasNextLong()) return;\n        long C = sc.nextLong();\n        long R = sc.nextLong();\n        int N = sc.nextInt();\n        long[] t = new long[N];\n        for (int i = 0; i < N; i++) t[i] = sc.nextLong();\n        \n        // Your logic here\n    }\n}\n".to_string());
                m
            },
        },

        // Question 2
        Question {
            id: "q2-subnet-allocator".to_string(),
            number: 2,
            title: "Campus Subnet IP Pool Allocator".to_string(),
            difficulty: "Medium".to_string(),
            points: 35,
            tags: vec!["Networking".to_string(), "Greedy".to_string(), "Bit Manipulation".to_string()],
            description: "A college campus network administrator needs to allocate contiguous IPv4 subnet blocks from an available CIDR network block to `M` campus departments. Each department `i` requires at least `H_i` usable host IP addresses.\n\nRecall that for a subnet with prefix length `P` (where `0 <= P <= 32`), the total number of IP addresses is `2^(32 - P)`. Since the network address and broadcast address are reserved, the number of usable hosts is `2^(32 - P) - 2`.\n\nYour task is to compute the minimum total number of IP addresses (usable + overhead) required to satisfy all departments without overlapping, assuming standard power-of-two subnetting (VLSM - Variable Length Subnet Masking).".to_string(),
            input_format: "Line 1: An integer M (number of departments).\nLine 2: M space-separated integers representing host requirements H_1, H_2, ..., H_M.".to_string(),
            output_format: "A single integer: the minimum total addresses allocated across all departments.".to_string(),
            constraints: vec![
                "1 <= M <= 100,000".to_string(),
                "1 <= H_i <= 10,000,000".to_string(),
            ],
            sample_cases: vec![
                TestCase {
                    input: "3\n50 10 120".to_string(),
                    expected_output: "264".to_string(),
                    explanation: Some("For H=50: need 50+2=52 addresses -> next power of 2 is 64.\nFor H=10: need 10+2=12 addresses -> next power of 2 is 16.\nFor H=120: need 120+2=122 addresses -> next power of 2 is 128.\nTotal addresses = 64 + 16 + 128 = 208 (or with alignment = 208).".to_string()),
                },
            ],
            starter_templates: {
                let mut m = HashMap::new();
                m.insert("python".to_string(), "# Solution for Campus Subnet IP Pool Allocator\nimport sys\n\ndef solve():\n    lines = sys.stdin.read().split()\n    if not lines:\n        return\n    M = int(lines[0])\n    H = [int(x) for x in lines[1:1+M]]\n    \n    # Your logic here\n    total_allocated = 0\n    for req in H:\n        needed = req + 2\n        # Next power of 2 >= needed\n        p = 1\n        while p < needed:\n            p <<= 1\n        total_allocated += p\n    print(total_allocated)\n\nif __name__ == '__main__':\n    solve()\n".to_string());
                m.insert("cpp".to_string(), "// Solution for Campus Subnet IP Pool Allocator\n#include <iostream>\n#include <vector>\n\nusing namespace std;\n\nint main() {\n    int M;\n    if (!(cin >> M)) return 0;\n    long long total = 0;\n    for (int i = 0; i < M; ++i) {\n        long long h;\n        cin >> h;\n        long long needed = h + 2;\n        long long p = 1;\n        while (p < needed) p <<= 1;\n        total += p;\n    }\n    cout << total << endl;\n    return 0;\n}\n".to_string());
                m.insert("java".to_string(), "// Solution for Campus Subnet IP Pool Allocator\nimport java.util.Scanner;\n\npublic class Solution {\n    public static void main(String[] args) {\n        Scanner sc = new Scanner(System.in);\n        if (!sc.hasNextInt()) return;\n        int M = sc.nextInt();\n        long total = 0;\n        for (int i = 0; i < M; i++) {\n            long h = sc.nextLong();\n            long needed = h + 2;\n            long p = 1;\n            while (p < needed) p <<= 1;\n            total += p;\n        }\n        System.out.println(total);\n    }\n}\n".to_string());
                m
            },
        },

        // Question 3
        Question {
            id: "q3-keystroke-outliers".to_string(),
            number: 3,
            title: "Keystroke Timing Outlier Detection".to_string(),
            difficulty: "Hard".to_string(),
            points: 35,
            tags: vec!["Data Analytics".to_string(), "Statistics".to_string(), "Security".to_string()],
            description: "CITADEL's proctoring sensor measures inter-keystroke intervals (time elapsed between successive key presses, in milliseconds). Human typing follows a log-normal distribution with high variance.\n\nConversely, automated paste scripts or macro injection bots produce unnaturally uniform delays with near-zero standard deviation.\n\nGiven `K` intervals `[d_1, d_2, ..., d_k]` and a threshold `EPS`, find the length of the longest contiguous subsegment of keystrokes where every interval is within `EPS` of the subsegment's median value. If this subsegment has length >= 10, flag it as anomalous (`BOT`), otherwise `HUMAN`.".to_string(),
            input_format: "Line 1: Two integers K (number of intervals) and EPS (tolerance threshold in ms).\nLine 2: K space-separated integers representing inter-keystroke delays in ms.".to_string(),
            output_format: "Line 1: 'BOT' or 'HUMAN'\nLine 2: Maximum uniform streak length found.".to_string(),
            constraints: vec![
                "1 <= K <= 200,000".to_string(),
                "0 <= EPS <= 50".to_string(),
                "1 <= d_i <= 5,000".to_string(),
            ],
            sample_cases: vec![
                TestCase {
                    input: "12 2\n120 121 119 120 120 121 120 120 119 121 120 450".to_string(),
                    expected_output: "BOT\n11".to_string(),
                    explanation: Some("The first 11 intervals are all between 119ms and 121ms (within EPS=2 of median 120). Length 11 >= 10, so classified as BOT.".to_string()),
                },
                TestCase {
                    input: "6 5\n120 340 180 220 90 400".to_string(),
                    expected_output: "HUMAN\n1".to_string(),
                    explanation: Some("High natural typing variance, max uniform streak is 1 < 10, classified as HUMAN.".to_string()),
                },
            ],
            starter_templates: {
                let mut m = HashMap::new();
                m.insert("python".to_string(), "# Solution for Keystroke Timing Outlier Detection\nimport sys\n\ndef solve():\n    lines = sys.stdin.read().split()\n    if not lines:\n        return\n    K = int(lines[0])\n    EPS = int(lines[1])\n    delays = [int(x) for x in lines[2:2+K]]\n    \n    # Your logic here\n    # Output BOT or HUMAN, followed by max length\n    pass\n\nif __name__ == '__main__':\n    solve()\n".to_string());
                m.insert("cpp".to_string(), "// Solution for Keystroke Timing Outlier Detection\n#include <iostream>\n#include <vector>\n\nusing namespace std;\n\nint main() {\n    int K, EPS;\n    if (!(cin >> K >> EPS)) return 0;\n    vector<int> d(K);\n    for (int i = 0; i < K; ++i) cin >> d[i];\n    \n    // Your logic here\n    return 0;\n}\n".to_string());
                m.insert("java".to_string(), "// Solution for Keystroke Timing Outlier Detection\nimport java.util.Scanner;\n\npublic class Solution {\n    public static void main(String[] args) {\n        Scanner sc = new Scanner(System.in);\n        if (!sc.hasNextInt()) return;\n        int K = sc.nextInt();\n        int EPS = sc.nextInt();\n        int[] d = new int[K];\n        for (int i = 0; i < K; i++) d[i] = sc.nextInt();\n        \n        // Your logic here\n    }\n}\n".to_string());
                m
            },
        },
    ]
}

pub fn get_question_summaries() -> Vec<QuestionSummary> {
    get_all_questions()
        .into_iter()
        .map(|q| QuestionSummary {
            id: q.id,
            number: q.number,
            title: q.title,
            difficulty: q.difficulty,
            points: q.points,
            tags: q.tags,
        })
        .collect()
}

pub fn get_question_by_id(id: &str) -> Option<Question> {
    get_all_questions().into_iter().find(|q| q.id == id)
}
