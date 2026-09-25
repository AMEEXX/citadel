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
    #[serde(default)]
    pub hidden_cases: Vec<TestCase>,
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
            "Attempt all 3 questions. Your code will be automatically compiled and validated against both sample and hidden test cases.".to_string(),
            "Switching windows, minimizing the kiosk, or attempting copy/paste triggers security integrity events logged on the proctor dashboard.".to_string(),
            "Click 'Run Sample Tests' to verify sample cases, and 'Submit Final Code' when you are confident in your solution.".to_string(),
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
            difficulty: "Easy".to_string(),
            points: 30,
            tags: vec!["Algorithms".to_string(), "Data Structures".to_string(), "Systems".to_string()],
            description: "You are designing the request rate limiter for CITADEL's campus assessment servers. Each candidate device has a token bucket of capacity `C` tokens, initially filled with `C` tokens. Tokens refill at a constant rate of `R` tokens per second (up to a maximum capacity of `C`).\n\nYou are given a stream of `N` incoming requests from a candidate device. Each request is described by two integers: `t_i` (the timestamp in seconds when the request arrives) and `cost_i` (number of tokens required to process the request).\n\nFor each request, output `PASS` if there are at least `cost_i` tokens available in the bucket at timestamp `t_i`, in which case those tokens are consumed. Otherwise output `DROP`, and no tokens are consumed.".to_string(),
            input_format: "Line 1: Three integers C (capacity), R (refill rate per second), and N (number of requests).\nNext N lines: Two integers t_i and cost_i representing arrival time and token cost.".to_string(),
            output_format: "N lines, each containing either 'PASS' or 'DROP'.".to_string(),
            constraints: vec![
                "1 <= C <= 1,000,000".to_string(),
                "1 <= R <= 100,000".to_string(),
                "1 <= N <= 100,000".to_string(),
                "0 <= t_1 <= t_2 <= ... <= t_N <= 10^9".to_string(),
                "1 <= cost_i <= C".to_string(),
            ],
            sample_cases: vec![
                TestCase {
                    input: "10 2 4\n0 5\n1 5\n2 5\n3 2".to_string(),
                    expected_output: "PASS\nPASS\nDROP\nPASS".to_string(),
                    explanation: Some("At t=0: bucket has 10 tokens. Cost 5 -> PASS, remaining = 5.\nAt t=1: refilled by (1-0)*2=2 -> 7 tokens. Cost 5 -> PASS, remaining = 2.\nAt t=2: refilled by (2-1)*2=2 -> 4 tokens. Cost 5 > 4 -> DROP, remaining = 4.\nAt t=3: refilled by (3-2)*2=2 -> 6 tokens. Cost 2 -> PASS, remaining = 4.".to_string()),
                },
                TestCase {
                    input: "5 1 3\n0 6\n1 1\n5 5".to_string(),
                    expected_output: "DROP\nPASS\nPASS".to_string(),
                    explanation: Some("Cost 6 > capacity 5 -> DROP immediately.".to_string()),
                },
            ],
            hidden_cases: vec![
                TestCase {
                    input: "20 5 3\n0 15\n1 10\n2 10".to_string(),
                    expected_output: "PASS\nPASS\nPASS".to_string(),
                    explanation: None,
                },
                TestCase {
                    input: "10 1 3\n0 10\n1 2\n2 2".to_string(),
                    expected_output: "PASS\nDROP\nPASS".to_string(),
                    explanation: None,
                },
                TestCase {
                    input: "100 10 2\n0 100\n10 100".to_string(),
                    expected_output: "PASS\nPASS".to_string(),
                    explanation: None,
                },
            ],
            starter_templates: {
                let mut m = HashMap::new();
                m.insert("python".to_string(), r#"# Solution for Distributed Token Bucket Rate Limiter
import sys

def solve():
    lines = sys.stdin.read().split()
    if not lines:
        return
    C = int(lines[0])
    R = int(lines[1])
    N = int(lines[2])
    
    current_tokens = C
    last_time = 0
    idx = 3
    
    for _ in range(N):
        t = int(lines[idx])
        cost = int(lines[idx+1])
        idx += 2
        
        elapsed = t - last_time
        current_tokens = min(C, current_tokens + elapsed * R)
        last_time = t
        
        if current_tokens >= cost:
            current_tokens -= cost
            print('PASS')
        else:
            print('DROP')

if __name__ == '__main__':
    solve()
"#.to_string());
                m.insert("cpp".to_string(), r#"// Solution for Distributed Token Bucket Rate Limiter
#include <iostream>
#include <algorithm>

using namespace std;

int main() {
    long long C, R;
    int N;
    if (!(cin >> C >> R >> N)) return 0;
    
    long long current_tokens = C;
    long long last_time = 0;
    
    for (int i = 0; i < N; ++i) {
        long long t, cost;
        cin >> t >> cost;
        long long elapsed = t - last_time;
        current_tokens = min(C, current_tokens + elapsed * R);
        last_time = t;
        
        if (current_tokens >= cost) {
            current_tokens -= cost;
            cout << "PASS\n";
        } else {
            cout << "DROP\n";
        }
    }
    return 0;
}
"#.to_string());
                m.insert("java".to_string(), r#"// Solution for Distributed Token Bucket Rate Limiter
import java.util.Scanner;

public class Solution {
    public static void main(String[] args) {
        Scanner sc = new Scanner(System.in);
        if (!sc.hasNextLong()) return;
        long C = sc.nextLong();
        long R = sc.nextLong();
        int N = sc.nextInt();
        
        long currentTokens = C;
        long lastTime = 0;
        
        for (int i = 0; i < N; i++) {
            long t = sc.nextLong();
            long cost = sc.nextLong();
            long elapsed = t - lastTime;
            currentTokens = Math.min(C, currentTokens + elapsed * R);
            lastTime = t;
            
            if (currentTokens >= cost) {
                currentTokens -= cost;
                System.out.println("PASS");
            } else {
                System.out.println("DROP");
            }
        }
    }
}
"#.to_string());
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
                    expected_output: "208".to_string(),
                    explanation: Some("For H=50: need 50+2=52 addresses -> next power of 2 is 64.\nFor H=10: need 10+2=12 addresses -> next power of 2 is 16.\nFor H=120: need 120+2=122 addresses -> next power of 2 is 128.\nTotal addresses = 64 + 16 + 128 = 208.".to_string()),
                },
            ],
            hidden_cases: vec![
                TestCase {
                    input: "1\n1".to_string(),
                    expected_output: "4".to_string(),
                    explanation: None,
                },
                TestCase {
                    input: "2\n30 30".to_string(),
                    expected_output: "64".to_string(),
                    explanation: None,
                },
                TestCase {
                    input: "4\n6 6 6 6".to_string(),
                    expected_output: "64".to_string(),
                    explanation: None,
                },
            ],
            starter_templates: {
                let mut m = HashMap::new();
                m.insert("python".to_string(), r#"# Solution for Campus Subnet IP Pool Allocator
import sys

def solve():
    lines = sys.stdin.read().split()
    if not lines:
        return
    M = int(lines[0])
    H = [int(x) for x in lines[1:1+M]]
    
    total_allocated = 0
    for req in H:
        needed = req + 2
        p = 1
        while p < needed:
            p <<= 1
        total_allocated += p
    print(total_allocated)

if __name__ == '__main__':
    solve()
"#.to_string());
                m.insert("cpp".to_string(), r#"// Solution for Campus Subnet IP Pool Allocator
#include <iostream>
#include <vector>

using namespace std;

int main() {
    int M;
    if (!(cin >> M)) return 0;
    long long total = 0;
    for (int i = 0; i < M; ++i) {
        long long h;
        cin >> h;
        long long needed = h + 2;
        long long p = 1;
        while (p < needed) p <<= 1;
        total += p;
    }
    cout << total << endl;
    return 0;
}
"#.to_string());
                m.insert("java".to_string(), r#"// Solution for Campus Subnet IP Pool Allocator
import java.util.Scanner;

public class Solution {
    public static void main(String[] args) {
        Scanner sc = new Scanner(System.in);
        if (!sc.hasNextInt()) return;
        int M = sc.nextInt();
        long total = 0;
        for (int i = 0; i < M; i++) {
            long h = sc.nextLong();
            long needed = h + 2;
            long p = 1;
            while (p < needed) p <<= 1;
            total += p;
        }
        System.out.println(total);
    }
}
"#.to_string());
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
            description: "CITADEL's proctoring sensor measures inter-keystroke intervals (time elapsed between successive key presses, in milliseconds). Human typing follows a log-normal distribution with high variance.\n\nConversely, automated paste scripts or macro injection bots produce unnaturally uniform delays with near-zero standard deviation.\n\nGiven `K` intervals `[d_1, d_2, ..., d_k]` and a threshold `EPS`, find the length of the longest contiguous subsegment of keystrokes where every interval is within `EPS` of the first interval in that subsegment. If this subsegment has length >= 10, flag it as anomalous (`BOT`), otherwise `HUMAN`.".to_string(),
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
                    explanation: Some("The first 11 intervals are all within EPS=2 of the initial 120. Length 11 >= 10, so classified as BOT.".to_string()),
                },
                TestCase {
                    input: "6 5\n120 340 180 220 90 400".to_string(),
                    expected_output: "HUMAN\n1".to_string(),
                    explanation: Some("High natural typing variance, max uniform streak is 1 < 10, classified as HUMAN.".to_string()),
                },
            ],
            hidden_cases: vec![
                TestCase {
                    input: "10 0\n50 50 50 50 50 50 50 50 50 50".to_string(),
                    expected_output: "BOT\n10".to_string(),
                    explanation: None,
                },
                TestCase {
                    input: "5 10\n100 200 300 400 500".to_string(),
                    expected_output: "HUMAN\n1".to_string(),
                    explanation: None,
                },
                TestCase {
                    input: "15 5\n200 205 195 200 202 201 198 199 203 204 200 201 500 500 500".to_string(),
                    expected_output: "BOT\n12".to_string(),
                    explanation: None,
                },
            ],
            starter_templates: {
                let mut m = HashMap::new();
                m.insert("python".to_string(), r#"# Solution for Keystroke Timing Outlier Detection
import sys

def solve():
    lines = sys.stdin.read().split()
    if not lines:
        return
    K = int(lines[0])
    EPS = int(lines[1])
    delays = [int(x) for x in lines[2:2+K]]
    
    max_streak = 0
    curr_streak = 0
    base_val = None
    
    for d in delays:
        if base_val is None:
            base_val = d
            curr_streak = 1
        elif abs(d - base_val) <= EPS:
            curr_streak += 1
        else:
            base_val = d
            curr_streak = 1
        if curr_streak > max_streak:
            max_streak = curr_streak
            
    if max_streak >= 10:
        print('BOT')
    else:
        print('HUMAN')
    print(max_streak)

if __name__ == '__main__':
    solve()
"#.to_string());
                m.insert("cpp".to_string(), r#"// Solution for Keystroke Timing Outlier Detection
#include <iostream>
#include <vector>
#include <cmath>

using namespace std;

int main() {
    int K, EPS;
    if (!(cin >> K >> EPS)) return 0;
    vector<int> d(K);
    for (int i = 0; i < K; ++i) cin >> d[i];
    
    int max_streak = 0;
    int curr_streak = 0;
    int base_val = 0;
    
    for (int i = 0; i < K; ++i) {
        if (curr_streak == 0) {
            base_val = d[i];
            curr_streak = 1;
        } else if (abs(d[i] - base_val) <= EPS) {
            curr_streak++;
        } else {
            base_val = d[i];
            curr_streak = 1;
        }
        if (curr_streak > max_streak) max_streak = curr_streak;
    }
    
    if (max_streak >= 10) cout << "BOT\n";
    else cout << "HUMAN\n";
    cout << max_streak << endl;
    return 0;
}
"#.to_string());
                m.insert("java".to_string(), r#"// Solution for Keystroke Timing Outlier Detection
import java.util.Scanner;

public class Solution {
    public static void main(String[] args) {
        Scanner sc = new Scanner(System.in);
        if (!sc.hasNextInt()) return;
        int K = sc.nextInt();
        int EPS = sc.nextInt();
        int[] d = new int[K];
        for (int i = 0; i < K; i++) d[i] = sc.nextInt();
        
        int maxStreak = 0;
        int currStreak = 0;
        int baseVal = 0;
        
        for (int i = 0; i < K; i++) {
            if (currStreak == 0) {
                baseVal = d[i];
                currStreak = 1;
            } else if (Math.abs(d[i] - baseVal) <= EPS) {
                currStreak++;
            } else {
                baseVal = d[i];
                currStreak = 1;
            }
            if (currStreak > maxStreak) maxStreak = currStreak;
        }
        
        if (maxStreak >= 10) System.out.println("BOT");
        else System.out.println("HUMAN");
        System.out.println(maxStreak);
    }
}
"#.to_string());
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

pub fn sanitize_for_candidate(mut q: Question) -> Question {
    q.hidden_cases.clear();
    q
}
