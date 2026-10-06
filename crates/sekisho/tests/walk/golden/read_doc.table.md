| principal | principal.department | resource.department | resource.owner is principal | resource.tenant is principal.tenant | principal in resource.team | -> | policies |
|---|---|---|---|---|---|---|---|
| User | sales | support | yes | yes | no | allow | owners_read |
| User | any | any | yes | no | any | allow | owners_read |
| User | support | sales | yes | yes | no | allow | owners_read |
| User | sales | support | yes | yes | yes | allow | owners_read, team_reads_within_the_tenant |
| User | support | sales | yes | yes | yes | allow | owners_read, team_reads_within_the_tenant |
| User | sales | sales | yes | yes | yes | allow | owners_read, team_reads_within_the_tenant, department_reads_within_the_tenant |
| User | support | support | yes | yes | yes | allow | owners_read, team_reads_within_the_tenant, department_reads_within_the_tenant |
| User | sales | sales | yes | yes | no | allow | owners_read, department_reads_within_the_tenant |
| User | support | support | yes | yes | no | allow | owners_read, department_reads_within_the_tenant |
| User | sales | support | no | yes | yes | allow | team_reads_within_the_tenant |
| User | support | sales | no | yes | yes | allow | team_reads_within_the_tenant |
| User | sales | sales | no | yes | yes | allow | team_reads_within_the_tenant, department_reads_within_the_tenant |
| User | support | support | no | yes | yes | allow | team_reads_within_the_tenant, department_reads_within_the_tenant |
| User | sales | sales | no | yes | no | allow | department_reads_within_the_tenant |
| User | support | support | no | yes | no | allow | department_reads_within_the_tenant |
| User | sales | support | no | yes | no | deny | (no permit) |
| User | any | any | no | no | any | deny | (no permit) |
| User | support | sales | no | yes | no | deny | (no permit) |
