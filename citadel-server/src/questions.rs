use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TestCase {
    pub input: String,
    pub expected_output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
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
        total_questions: 1,
        total_points: 100,
        instructions: vec![
            "This assessment runs on a zero-internet isolated local network. No external browsing is permitted.".to_string(),
            "Solve the Two Sum algorithmic problem using Python, C++, or Java.".to_string(),
            "Click 'Run Code' to verify your solution against the sample test cases.".to_string(),
            "Once all sample test cases pass, the Submit button unlocks to grade your solution against the hidden test suite.".to_string(),
            "Do not attempt Alt+Tab, three-finger touchpad gestures, or opening secondary windows; all violations are logged.".to_string(),
        ],
    }
}

pub fn get_all_questions() -> Vec<Question> {
    vec![
        Question {
            id: "q1-two-sum".to_string(),
            number: 1,
            title: "Two Sum".to_string(),
            difficulty: "Easy".to_string(),
            points: 100,
            tags: vec!["Array".to_string(), "Hash Table".to_string(), "Two Pointers".to_string()],
            description: r#"Given an array of integers <code>nums</code> and an integer <code>target</code>, return <strong>indices of the two numbers</strong> such that they add up to <code>target</code>.

<p style="margin-top: 10px;">You may assume that each input would have <strong>exactly one solution</strong>, and you may not use the same element twice.</p>
<p style="margin-top: 6px;">You can return the answer in any order (indices separated by space).</p>"#.to_string(),
            input_format: "The standard input contains space-separated integers representing the array <code>nums</code> followed by the <code>target</code> value as the final integer.
Example: <code>2 7 11 15 9</code> means <code>nums = [2, 7, 11, 15]</code> and <code>target = 9</code>.".to_string(),
            output_format: "Output the two zero-based indices separated by a space on standard output (e.g. <code>0 1</code>).".to_string(),
            constraints: vec![
                "2 <= nums.length <= 10^4".to_string(),
                "-10^9 <= nums[i] <= 10^9".to_string(),
                "-10^9 <= target <= 10^9".to_string(),
                "Only one valid answer exists for each test case.".to_string(),
            ],
            sample_cases: vec![
                TestCase {
                    input: "2 7 11 15 9".to_string(),
                    expected_output: "0 1".to_string(),
                    explanation: Some("Because nums[0] + nums[1] == 2 + 7 == 9, we return indices 0 1.".to_string()),
                },
                TestCase {
                    input: "3 2 4 6".to_string(),
                    expected_output: "1 2".to_string(),
                    explanation: Some("Because nums[1] + nums[2] == 2 + 4 == 6, we return indices 1 2.".to_string()),
                },
                TestCase {
                    input: "3 3 6".to_string(),
                    expected_output: "0 1".to_string(),
                    explanation: Some("Because nums[0] + nums[1] == 3 + 3 == 6, we return indices 0 1.".to_string()),
                },
            ],
            hidden_cases: vec![
                TestCase {
                    input: "-1 -2 -3 -4 -5 -8".to_string(),
                    expected_output: "2 4".to_string(),
                    explanation: None,
                },
                TestCase {
                    input: "1 5 20 30 40 99 100".to_string(),
                    expected_output: "0 5".to_string(),
                    explanation: None,
                },
                TestCase {
                    input: "0 4 3 0 0".to_string(),
                    expected_output: "0 3".to_string(),
                    explanation: None,
                },
                TestCase {
                    input: "1 2 3 4 4 9 8".to_string(),
                    expected_output: "3 4".to_string(),
                    explanation: None,
                },
                TestCase {
                    input: "1000000000 500 1000000000 2000000000".to_string(),
                    expected_output: "0 2".to_string(),
                    explanation: None,
                },
            ],
            starter_templates: {
                let mut m = HashMap::new();
                m.insert("python".to_string(), r#"import sys

def two_sum(nums, target):
    # Write your solution here
    # Return a list containing the two indices [i, j]
    pass

if __name__ == '__main__':
    tokens = sys.stdin.read().split()
    if tokens:
        target = int(tokens[-1])
        nums = [int(x) for x in tokens[:-1]]
        ans = two_sum(nums, target)
        if ans and len(ans) == 2:
            print(f"{min(ans[0], ans[1])} {max(ans[0], ans[1])}")
"#.to_string());

                m.insert("cpp".to_string(), r#"#include <iostream>
#include <vector>
#include <unordered_map>
#include <algorithm>

using namespace std;

vector<int> twoSum(vector<int>& nums, int target) {
    // Write your solution here
    // Return vector containing the two indices
    return {};
}

int main() {
    ios_base::sync_with_stdio(false);
    cin.tie(NULL);

    vector<int> nums;
    int val;
    while (cin >> val) {
        nums.push_back(val);
    }
    if (nums.size() >= 2) {
        int target = nums.back();
        nums.pop_back();
        vector<int> ans = twoSum(nums, target);
        if (ans.size() == 2) {
            cout << min(ans[0], ans[1]) << " " << max(ans[0], ans[1]) << "
";
        }
    }
    return 0;
}
"#.to_string());

                m.insert("java".to_string(), r#"import java.util.*;

public class Solution {
    public static int[] twoSum(int[] nums, int target) {
        // Write your solution here
        // Return int array with two indices
        return new int[]{};
    }

    public static void main(String[] args) {
        Scanner sc = new Scanner(System.in);
        List<Integer> list = new ArrayList<>();
        while (sc.hasNextInt()) {
            list.add(sc.nextInt());
        }
        if (list.size() >= 2) {
            int target = list.remove(list.size() - 1);
            int[] nums = new int[list.size()];
            for (int i = 0; i < list.size(); i++) {
                nums[i] = list.get(i);
            }
            int[] ans = twoSum(nums, target);
            if (ans != null && ans.length == 2) {
                System.out.println(Math.min(ans[0], ans[1]) + " " + Math.max(ans[0], ans[1]));
            }
        }
    }
}
"#.to_string());
                m
            },
        },
    ]
}

pub fn get_question_summaries(questions: &[Question]) -> Vec<QuestionSummary> {
    questions
        .iter()
        .map(|q| QuestionSummary {
            id: q.id.clone(),
            number: q.number,
            title: q.title.clone(),
            difficulty: q.difficulty.clone(),
            points: q.points,
            tags: q.tags.clone(),
        })
        .collect()
}

pub fn sanitize_for_candidate(mut q: Question) -> Question {
    q.hidden_cases.clear();
    q
}
